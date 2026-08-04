use codec::Encode;
use native_api::Api;
use pallet_tables::{CommitmentCreationCmd, UpdateTable};
use polkadot_sdk::frame_support::traits::Hooks;
use polkadot_sdk::frame_support::BoundedVec;
use polkadot_sdk::frame_system::EventRecord;
use polkadot_sdk::sp_core::offchain::testing::{PendingRequest, TestOffchainExt};
use polkadot_sdk::sp_core::offchain::{OffchainDbExt, OffchainWorkerExt};
use polkadot_sdk::sp_core::storage::StorageData;
use polkadot_sdk::sp_core::H256;
use polkadot_sdk::sp_runtime::offchain::storage_lock::{StorageLock, Time};
use polkadot_sdk::sp_runtime::offchain::Duration;
use proof_of_sql_commitment_map::CommitmentSchemeFlags;
use prost::Message;
use sxt_core::indexing::{BatchId, DataQuorum, SubmitterList};
use sxt_core::prover_db_indexer::{PROVER_DB_CONFIG_INCLUDE_KEY, PROVER_DB_CONFIG_URL_KEY};
use sxt_core::tables::{QuorumScope, Source, TableIdentifier, TableType};

use crate::mock::*;
use crate::proto;

type StateArc =
    std::sync::Arc<parking_lot::RwLock<polkadot_sdk::sp_core::offchain::testing::OffchainState>>;

const MOCK_URL: &str = "http://127.0.0.1:9999";

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

fn setup_with_config(
    filters: &str,
    finalized_block_num: u32,
    events: impl IntoIterator<Item = Vec<EventRecord<RuntimeEvent, H256>>>,
) -> (polkadot_sdk::sp_io::TestExternalities, StateArc) {
    let mut ext = new_test_ext();
    let (offchain, state) = TestOffchainExt::new();
    ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
    ext.register_extension(OffchainDbExt::new(offchain));
    ext.register_extension(MockClientProvider::client_ext(
        Some((H256::zero(), finalized_block_num)),
        events
            .into_iter()
            .map(|e| Ok(Some(StorageData(e.encode())))),
    ));
    let mut config_store = std::collections::HashMap::new();
    config_store.insert(PROVER_DB_CONFIG_URL_KEY.to_string(), MOCK_URL.to_string());
    config_store.insert(
        PROVER_DB_CONFIG_INCLUDE_KEY.to_string(),
        filters.to_string(),
    );
    ext.register_extension(native::config::ConfigExt(std::sync::Arc::new(config_store)));
    (ext, state)
}

fn schema_updated_event(
    name_namespace_ddl_tuples: impl IntoIterator<Item = (&'static str, &'static str, Vec<u8>)>,
) -> EventRecord<RuntimeEvent, H256> {
    event_record(pallet_tables::Event::<Test>::SchemaUpdated(
        None,
        BoundedVec::truncate_from(
            name_namespace_ddl_tuples
                .into_iter()
                .map(|(name, namespace, ddl)| UpdateTable {
                    ident: TableIdentifier::from_str_unchecked(name, namespace),
                    create_statement: BoundedVec::truncate_from(ddl),
                    table_type: TableType::default(),
                    commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::default()),
                    source: Source::default(),
                })
                .collect(),
        ),
    ))
}
fn table_dropped_event(name: &str, namespace: &str) -> EventRecord<RuntimeEvent, H256> {
    event_record(pallet_tables::Event::<Test>::TableDropped(
        None,
        TableType::default(),
        TableIdentifier::from_str_unchecked(name, namespace),
        Source::default(),
    ))
}
fn quorum_reached_event(
    name: &str,
    namespace: &str,
    data: Vec<u8>,
) -> EventRecord<RuntimeEvent, H256> {
    event_record(pallet_indexing::Event::<Test, Api>::QuorumReached {
        quorum: DataQuorum {
            table: TableIdentifier::from_str_unchecked(name, namespace),
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
    let (mut ext, _state) = setup_with_config("*.*", 1, [vec![]]);

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
    let ddl = b"CREATE TABLE PUBLIC.USERS (ID BIGINT NOT NULL)";
    let event = schema_updated_event([("USERS", "PUBLIC", ddl.to_vec())]);
    let (mut ext, state) = setup_with_config("*.*", 1, [vec![event]]);

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
    let (mut ext, state) = setup_with_config("*.*", 1, [vec![]]);

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
    let event = table_dropped_event("OLD", "NS");
    let (mut ext, state) = setup_with_config("*.*", 6, [vec![event]]);

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
    let event_1 = table_dropped_event("T1", "NS");
    let event_2 =
        schema_updated_event([("T2", "NS", b"CREATE TABLE NS.T2 (X INT NOT NULL)".to_vec())]);
    let (mut ext, state) = setup_with_config("*.*", 3, [vec![event_1], vec![event_2], vec![]]);

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
    let event_1 = table_dropped_event("T1", "NS");
    let event_2 = quorum_reached_event("T2", "NS", b"row-data".to_vec());
    let (mut ext, state) = setup_with_config("*.*", 1, [vec![event_1, event_2]]);

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
    let event_1 = schema_updated_event([("A", "ALPHA", b"CREATE TABLE ...".to_vec())]);
    let event_2 = quorum_reached_event("BETA_T", "BETA_NS", b"rows".to_vec());
    let event_3 = table_dropped_event("GAMMA", "OTHER");
    let event_4 = schema_updated_event([("GAMMA", "X", b"CREATE TABLE ...".to_vec())]);
    let event_5 = quorum_reached_event("Y", "ALPHA", b"rows".to_vec());
    let events = vec![event_1, event_2, event_3, event_4, event_5];
    let (mut ext, state) = setup_with_config("ALPHA.*,BETA_NS.BETA_T", 1, [events]);

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
    let ddl = b"CREATE TABLE ANY.ANY (ID BIGINT NOT NULL)";
    let event = schema_updated_event([("ANY", "ANY", ddl.to_vec())]);
    let (mut ext, state) = setup_with_config("*.*", 1, [vec![event]]);

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
    let ddl = b"CREATE TABLE ANY.ANY (ID BIGINT NOT NULL)";
    let event = schema_updated_event([("ANY", "ANY", ddl.to_vec())]);
    let (mut ext, state) = setup_with_config("", 1, [vec![event]]);

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
