//! Integration tests for the prover-db-indexer consumer (OCW).
//!
//! `forward_block` reads a block's captured events straight from the
//! node's client (`System::Events` at that block's hash) via
//! [`crate::db_events::db_events_at`], so these tests seed the mocked
//! client's responses instead of the old per-extrinsic offchain-DB queue.
//! `MockClientProvider` hands out one queued response per `storage` call,
//! and the OCW makes exactly one such call per block it walks, in
//! increasing block-number order — so the queue order lines up with the
//! block order these tests describe.

use std::borrow::Cow;

use codec::Encode;
use native_api::Api;
use pallet_tables::{CommitmentCreationCmd, UpdateTable};
use polkadot_sdk::frame_support::traits::Hooks;
use polkadot_sdk::frame_support::BoundedVec;
use polkadot_sdk::frame_system::EventRecord;
use polkadot_sdk::sp_core::offchain::testing::{PendingRequest, TestOffchainExt};
use polkadot_sdk::sp_core::offchain::{OffchainDbExt, OffchainStorage, OffchainWorkerExt};
use polkadot_sdk::sp_core::storage::StorageData;
use polkadot_sdk::sp_core::H256;
use polkadot_sdk::sp_runtime::offchain::storage_lock::{StorageLock, Time};
use polkadot_sdk::sp_runtime::offchain::Duration;
use proof_of_sql_commitment_map::CommitmentSchemeFlags;
use prost::Message;
use sxt_core::indexing::{BatchId, DataQuorum, SubmitterList};
use sxt_core::prover_db_indexer::{
    key_for_event,
    BlockEvent,
    CreateEntry,
    EventCapture,
    InsertEntry,
    ProverDbConsumerConfig,
    TableIdentifierFilter,
};
use sxt_core::tables::{QuorumScope, Source, TableIdentifier, TableType};

use crate::mock::*;
use crate::{proto, PROVER_DB_CONFIG_KEY};

type StateArc =
    std::sync::Arc<parking_lot::RwLock<polkadot_sdk::sp_core::offchain::testing::OffchainState>>;

const MOCK_URL: &str = "http://127.0.0.1:9999";

/// SCALE-encode the unified consumer config the same way the node
/// service does at startup.
fn encode_config(include: Vec<TableIdentifierFilter>) -> Vec<u8> {
    codec::Encode::encode(&ProverDbConsumerConfig {
        url: MOCK_URL.to_string(),
        include,
    })
}

fn expected_request(path: &str, body: Vec<u8>, response: Vec<u8>) -> PendingRequest {
    PendingRequest {
        method: "POST".into(),
        uri: format!("{}{}", MOCK_URL, path),
        headers: vec![("Content-Type".into(), "application/x-protobuf".into())],
        body,
        sent: true,
        response: Some(response),
        ..Default::default()
    }
}

fn checkpoint_at_response(seq: u64) -> Vec<u8> {
    proto::GetLastCheckpointResponse {
        sequence_number: seq,
        has_checkpoint: true,
    }
    .encode_to_vec()
}

fn no_checkpoint_response() -> Vec<u8> {
    proto::GetLastCheckpointResponse {
        sequence_number: 0,
        has_checkpoint: false,
    }
    .encode_to_vec()
}

/// Seed a `*.*` filter — what the operator gets when omitting
/// `--prover-db-include` (clap default). Every captured event should
/// reach the indexer.
fn setup_with_url() -> (polkadot_sdk::sp_io::TestExternalities, StateArc) {
    setup_with_config(vec!["*.*".parse().unwrap()])
}

/// Seed a non-empty include set under the unified config key. Used by
/// the consumer-side filter tests.
fn setup_with_config(
    include: Vec<TableIdentifierFilter>,
) -> (polkadot_sdk::sp_io::TestExternalities, StateArc) {
    let mut ext = new_test_ext();
    let (offchain, state) = TestOffchainExt::new();
    ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
    ext.register_extension(OffchainDbExt::new(offchain));
    state
        .write()
        .persistent_storage
        .set(b"", PROVER_DB_CONFIG_KEY, &encode_config(include));
    (ext, state)
}

fn schema_updated_event(
    identifier_create_statement_pairs: impl IntoIterator<Item = (TableIdentifier, Vec<u8>)>,
) -> EventRecord<RuntimeEvent, H256> {
    event_record(pallet_tables::Event::<Test>::SchemaUpdated(
        None,
        BoundedVec::truncate_from(
            identifier_create_statement_pairs
                .into_iter()
                .map(|(ident, ddl)| UpdateTable {
                    ident,
                    create_statement: BoundedVec::truncate_from(ddl),
                    table_type: TableType::default(),
                    commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::default()),
                    source: Source::default(),
                })
                .collect(),
        ),
    ))
}
fn table_dropped_event(table: TableIdentifier) -> EventRecord<RuntimeEvent, H256> {
    event_record(pallet_tables::Event::<Test>::TableDropped(
        None,
        TableType::default(),
        table,
        Source::default(),
    ))
}
fn quorum_reached_event(table: TableIdentifier, data: Vec<u8>) -> EventRecord<RuntimeEvent, H256> {
    event_record(pallet_indexing::Event::<Test, Api>::QuorumReached {
        quorum: DataQuorum {
            table,
            batch_id: BatchId::default(),
            data_hash: H256::zero(),
            block_number: Default::default(),
            agreements: SubmitterList::default(),
            dissents: SubmitterList::default(),
            quorum_scope: QuorumScope::Public,
        },
        data: BoundedVec::truncate_from(data),
    })
}

// ─── Tests ──────────────────────────────────────────────────────────────

#[test]
fn ocw_skips_when_not_configured() {
    let mut ext = new_test_ext();
    let (offchain, _) = TestOffchainExt::new();
    ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
    ext.register_extension(OffchainDbExt::new(offchain));
    ext.execute_with(|| {
        System::set_block_number(1);
        ProverDbIndexer::offchain_worker(1);
    });
}

/// If another OCW round is in progress (lock held), this invocation
/// must do nothing — no HTTP traffic, no state reads beyond the lock
/// itself. `TestOffchainExt` would panic on an unexpected request, so
/// the absence of queued expectations is the assertion.
#[test]
fn ocw_skips_when_lock_is_held() {
    let (mut ext, _state) = setup_with_url();
    ext.register_extension(MockClientProvider::client_ext(
        Some((H256::zero(), 1)),
        [Ok(None)],
    ));

    ext.execute_with(|| {
        let mut lock = StorageLock::<Time>::with_deadline(
            b"prover_db_indexer/ocw_lock",
            Duration::from_millis(120_000),
        );
        let _guard = lock.try_lock().expect("first acquisition must succeed");

        System::set_block_number(1);
        ProverDbIndexer::offchain_worker(1);
    });
}

#[test]
fn ocw_forwards_create_table_event() {
    let (mut ext, state) = setup_with_url();
    let ddl = b"CREATE TABLE PUBLIC.USERS (ID BIGINT NOT NULL)";

    let event = schema_updated_event([(
        TableIdentifier::from_str_unchecked("USERS", "PUBLIC"),
        ddl.to_vec(),
    )]);
    ext.register_extension(MockClientProvider::client_ext(
        Some((H256::zero(), 1)),
        [Ok(Some(StorageData(vec![event].encode())))],
    ));

    {
        let mut s = state.write();
        s.expect_request(expected_request(
            "/v1/get_last_checkpoint",
            vec![],
            no_checkpoint_response(),
        ));
        s.expect_request(expected_request(
            "/v1/create_table",
            proto::CreateTableRequest {
                sequence_number: 1,
                table_name: "PUBLIC.USERS".into(),
                arrow_schema: ddl.to_vec(),
                key: "META_ROW_NUMBER".into(),
            }
            .encode_to_vec(),
            vec![],
        ));
        s.expect_request(expected_request(
            "/v1/checkpoint",
            proto::CheckpointRequest { sequence_number: 1 }.encode_to_vec(),
            vec![],
        ));
    }

    ext.execute_with(|| {
        System::set_block_number(1);
        ProverDbIndexer::offchain_worker(1);
    });
}

#[test]
fn ocw_checkpoints_blocks_with_no_events() {
    let (mut ext, state) = setup_with_url();
    let storage_data = Vec::<EventRecord<RuntimeEvent, H256>>::new().encode();
    ext.register_extension(MockClientProvider::client_ext(
        Some((H256::zero(), 1)),
        [Ok(Some(StorageData(storage_data)))],
    ));

    {
        let mut s = state.write();
        s.expect_request(expected_request(
            "/v1/get_last_checkpoint",
            vec![],
            no_checkpoint_response(),
        ));
        s.expect_request(expected_request(
            "/v1/checkpoint",
            proto::CheckpointRequest { sequence_number: 1 }.encode_to_vec(),
            vec![],
        ));
    }

    ext.execute_with(|| {
        System::set_block_number(1);
        ProverDbIndexer::offchain_worker(1);
    });
}

#[test]
fn ocw_resumes_from_server_checkpoint() {
    let (mut ext, state) = setup_with_url();
    let event = table_dropped_event(TableIdentifier::from_str_unchecked("OLD", "NS"));
    ext.register_extension(MockClientProvider::client_ext(
        Some((H256::zero(), 6)),
        [Ok(Some(StorageData(vec![event].encode())))],
    ));

    {
        let mut s = state.write();
        s.expect_request(expected_request(
            "/v1/get_last_checkpoint",
            vec![],
            checkpoint_at_response(5),
        ));
        s.expect_request(expected_request(
            "/v1/drop_table",
            proto::DropTableRequest {
                sequence_number: 6,
                table_name: "NS.OLD".into(),
            }
            .encode_to_vec(),
            vec![],
        ));
        s.expect_request(expected_request(
            "/v1/checkpoint",
            proto::CheckpointRequest { sequence_number: 6 }.encode_to_vec(),
            vec![],
        ));
    }

    ext.execute_with(|| {
        System::set_block_number(6);
        ProverDbIndexer::offchain_worker(6);
    });
}

#[test]
fn ocw_processes_multiple_blocks_in_order() {
    let (mut ext, state) = setup_with_url();
    let event_1 = table_dropped_event(TableIdentifier::from_str_unchecked("T1", "NS"));
    let event_2 = schema_updated_event([(
        TableIdentifier::from_str_unchecked("T2", "NS"),
        b"CREATE TABLE NS.T2 (X INT NOT NULL)".to_vec(),
    )]);
    ext.register_extension(MockClientProvider::client_ext(
        Some((H256::zero(), 3)),
        [
            Ok(Some(StorageData(vec![event_1].encode()))),
            Ok(Some(StorageData(vec![event_2].encode()))),
            Ok(Some(StorageData(
                Vec::<EventRecord<RuntimeEvent, H256>>::new().encode(),
            ))),
        ],
    ));

    {
        let mut s = state.write();
        s.expect_request(expected_request(
            "/v1/get_last_checkpoint",
            vec![],
            no_checkpoint_response(),
        ));
        s.expect_request(expected_request(
            "/v1/drop_table",
            proto::DropTableRequest {
                sequence_number: 1,
                table_name: "NS.T1".into(),
            }
            .encode_to_vec(),
            vec![],
        ));
        s.expect_request(expected_request(
            "/v1/checkpoint",
            proto::CheckpointRequest { sequence_number: 1 }.encode_to_vec(),
            vec![],
        ));
        s.expect_request(expected_request(
            "/v1/create_table",
            proto::CreateTableRequest {
                sequence_number: 2,
                table_name: "NS.T2".into(),
                arrow_schema: b"CREATE TABLE NS.T2 (X INT NOT NULL)".to_vec(),
                key: "META_ROW_NUMBER".into(),
            }
            .encode_to_vec(),
            vec![],
        ));
        s.expect_request(expected_request(
            "/v1/checkpoint",
            proto::CheckpointRequest { sequence_number: 2 }.encode_to_vec(),
            vec![],
        ));
        s.expect_request(expected_request(
            "/v1/checkpoint",
            proto::CheckpointRequest { sequence_number: 3 }.encode_to_vec(),
            vec![],
        ));
    }

    ext.execute_with(|| {
        System::set_block_number(3);
        ProverDbIndexer::offchain_worker(3);
    });
}

#[test]
fn ocw_forwards_multiple_events_in_one_block() {
    let (mut ext, state) = setup_with_url();
    let event_1 = table_dropped_event(TableIdentifier::from_str_unchecked("T1", "NS"));
    let event_2 = quorum_reached_event(
        TableIdentifier::from_str_unchecked("T2", "NS"),
        b"row-data".to_vec(),
    );
    ext.register_extension(MockClientProvider::client_ext(
        Some((H256::zero(), 1)),
        [Ok(Some(StorageData(vec![event_1, event_2].encode())))],
    ));

    {
        let mut s = state.write();
        s.expect_request(expected_request(
            "/v1/get_last_checkpoint",
            vec![],
            no_checkpoint_response(),
        ));
        s.expect_request(expected_request(
            "/v1/drop_table",
            proto::DropTableRequest {
                sequence_number: 1,
                table_name: "NS.T1".into(),
            }
            .encode_to_vec(),
            vec![],
        ));
        s.expect_request(expected_request(
            "/v1/put_batches",
            proto::PutBatchesRequest {
                sequence_number: 1,
                batches: vec![proto::TableBatch {
                    table_name: "NS.T2".into(),
                    record_batch: b"row-data".to_vec(),
                }],
            }
            .encode_to_vec(),
            vec![],
        ));
        s.expect_request(expected_request(
            "/v1/checkpoint",
            proto::CheckpointRequest { sequence_number: 1 }.encode_to_vec(),
            vec![],
        ));
    }

    ext.execute_with(|| {
        System::set_block_number(1);
        ProverDbIndexer::offchain_worker(1);
    });
}

// ─── Include-set tests (consumer-side filter) ──────────────────────────

/// Helper: build a Drop event for `(name, namespace)`.
fn drop_event(name: &str, namespace: &str) -> BlockEvent<'static> {
    BlockEvent::Drop(Cow::Owned(TableIdentifier::from_str_unchecked(
        name, namespace,
    )))
}

/// Helper: build a Create event for `(name, namespace)`.
fn create_event(name: &str, namespace: &str) -> BlockEvent<'static> {
    BlockEvent::Create(CreateEntry {
        ident: Cow::Owned(TableIdentifier::from_str_unchecked(name, namespace)),
        ddl: Cow::Owned(b"CREATE TABLE ...".to_vec()),
    })
}

/// Helper: build an Insert event for `(name, namespace)`.
fn insert_event(name: &str, namespace: &str) -> BlockEvent<'static> {
    BlockEvent::Insert(InsertEntry {
        table: Cow::Owned(TableIdentifier::from_str_unchecked(name, namespace)),
        data: Cow::Owned(b"rows".to_vec()),
    })
}

/// `capture_events` is now unconditional — every event reaches the
/// offchain queue regardless of any per-node configuration. This test
/// is the single producer-side regression check.
#[test]
fn capture_events_writes_all_events_unconditionally() {
    let mut ext = new_test_ext();
    ext.execute_with(|| {
        System::set_block_number(1);
        polkadot_sdk::frame_system::Pallet::<Test>::set_extrinsic_index(0);

        let events = vec![
            create_event("A", "ALPHA"),
            insert_event("B", "BETA"),
            drop_event("C", "GAMMA"),
        ];
        <ProverDbIndexer as EventCapture>::capture_events(events);
    });
    ext.persist_offchain_overlay();
    let db = ext.offchain_db();
    let stored = db.get(&key_for_event(1, 0)).unwrap();
    let decoded: Vec<BlockEvent<'static>> = codec::Decode::decode(&mut &stored[..]).unwrap();
    assert_eq!(decoded.len(), 3);
}

/// End-to-end consumer test: seed the node's include set into offchain
/// local storage (where the OCW reads it), seed a multi-event block,
/// and confirm only the matching events are POSTed to the indexer.
#[test]
fn ocw_forwards_only_events_matching_include_set() {
    // Seed the node-local config with two include rules: any table in
    // namespace ALPHA, plus the specific table BETA_NS.BETA_T. The
    // service writes both URL + include atomically via a single
    // SCALE-encoded `ProverDbConsumerConfig` — same shape this test
    // mirrors.
    let (mut ext, state) = setup_with_config(vec![
        "ALPHA.*".parse().unwrap(),
        "BETA_NS.BETA_T".parse().unwrap(),
    ]);
    let event_1 = schema_updated_event([(
        TableIdentifier::from_str_unchecked("A", "ALPHA"),
        b"CREATE TABLE ...".to_vec(),
    )]);
    let event_2 = quorum_reached_event(
        TableIdentifier::from_str_unchecked("BETA_T", "BETA_NS"),
        b"rows".to_vec(),
    );
    let event_3 = table_dropped_event(TableIdentifier::from_str_unchecked("OTHER", "GAMMA"));
    let event_4 = schema_updated_event([(
        TableIdentifier::from_str_unchecked("X", "GAMMA"),
        b"CREATE TABLE ...".to_vec(),
    )]);
    let event_5 = quorum_reached_event(
        TableIdentifier::from_str_unchecked("Y", "ALPHA"),
        b"rows".to_vec(),
    );

    ext.register_extension(MockClientProvider::client_ext(
        Some((H256::zero(), 1)),
        [Ok(Some(StorageData(
            vec![event_1, event_2, event_3, event_4, event_5].encode(),
        )))],
    ));

    // Only the three matching events should reach the indexer. Drops
    // for the skipped GAMMA namespace get filtered too — uniform.
    {
        let mut s = state.write();
        s.expect_request(expected_request(
            "/v1/get_last_checkpoint",
            vec![],
            no_checkpoint_response(),
        ));
        s.expect_request(expected_request(
            "/v1/create_table",
            proto::CreateTableRequest {
                sequence_number: 1,
                table_name: "ALPHA.A".into(),
                arrow_schema: b"CREATE TABLE ...".to_vec(),
                key: "META_ROW_NUMBER".into(),
            }
            .encode_to_vec(),
            vec![],
        ));
        s.expect_request(expected_request(
            "/v1/put_batches",
            proto::PutBatchesRequest {
                sequence_number: 1,
                batches: vec![proto::TableBatch {
                    table_name: "BETA_NS.BETA_T".into(),
                    record_batch: b"rows".to_vec(),
                }],
            }
            .encode_to_vec(),
            vec![],
        ));
        s.expect_request(expected_request(
            "/v1/put_batches",
            proto::PutBatchesRequest {
                sequence_number: 1,
                batches: vec![proto::TableBatch {
                    table_name: "ALPHA.Y".into(),
                    record_batch: b"rows".to_vec(),
                }],
            }
            .encode_to_vec(),
            vec![],
        ));
        s.expect_request(expected_request(
            "/v1/checkpoint",
            proto::CheckpointRequest { sequence_number: 1 }.encode_to_vec(),
            vec![],
        ));
    }

    ext.execute_with(|| {
        System::set_block_number(1);
        ProverDbIndexer::offchain_worker(1);
    });
}

/// Explicit `*.*` filter in offchain storage ⇒ forward every event —
/// this is what the operator gets when omitting `--prover-db-include`
/// because the CLI default is `*.*`.
#[test]
fn ocw_with_wildcard_filter_forwards_everything() {
    let (mut ext, state) = setup_with_url();
    let ddl = b"CREATE TABLE ANY.ANY (ID BIGINT NOT NULL)";
    let event_1 = schema_updated_event([(
        TableIdentifier::from_str_unchecked("ANY", "ANY"),
        ddl.to_vec(),
    )]);
    ext.register_extension(MockClientProvider::client_ext(
        Some((H256::zero(), 1)),
        [Ok(Some(StorageData(vec![event_1].encode())))],
    ));

    {
        let mut s = state.write();
        s.expect_request(expected_request(
            "/v1/get_last_checkpoint",
            vec![],
            no_checkpoint_response(),
        ));
        s.expect_request(expected_request(
            "/v1/create_table",
            proto::CreateTableRequest {
                sequence_number: 1,
                table_name: "ANY.ANY".into(),
                arrow_schema: ddl.to_vec(),
                key: "META_ROW_NUMBER".into(),
            }
            .encode_to_vec(),
            vec![],
        ));
        s.expect_request(expected_request(
            "/v1/checkpoint",
            proto::CheckpointRequest { sequence_number: 1 }.encode_to_vec(),
            vec![],
        ));
    }

    ext.execute_with(|| {
        System::set_block_number(1);
        ProverDbIndexer::offchain_worker(1);
    });
}

/// Empty include set in offchain storage ⇒ forward nothing. The
/// checkpoint still advances (so the OCW makes server-side progress
/// past empty-as-far-as-this-node-is-concerned blocks), but no
/// `/v1/create_table` request is emitted. `TestOffchainExt` would
/// panic on an unexpected request, so the absence of that expectation
/// is the assertion.
#[test]
fn ocw_with_empty_include_set_forwards_nothing() {
    let (mut ext, state) = setup_with_config(Vec::new());
    let ddl = b"CREATE TABLE ANY.ANY (ID BIGINT NOT NULL)";
    let event_1 = schema_updated_event([(
        TableIdentifier::from_str_unchecked("ANY", "ANY"),
        ddl.to_vec(),
    )]);
    ext.register_extension(MockClientProvider::client_ext(
        Some((H256::zero(), 1)),
        [Ok(Some(StorageData(vec![event_1].encode())))],
    ));

    {
        let mut s = state.write();
        s.expect_request(expected_request(
            "/v1/get_last_checkpoint",
            vec![],
            no_checkpoint_response(),
        ));
        s.expect_request(expected_request(
            "/v1/checkpoint",
            proto::CheckpointRequest { sequence_number: 1 }.encode_to_vec(),
            vec![],
        ));
    }

    ext.execute_with(|| {
        System::set_block_number(1);
        ProverDbIndexer::offchain_worker(1);
    });
}
