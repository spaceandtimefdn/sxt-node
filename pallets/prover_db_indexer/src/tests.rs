//! Integration tests for the prover-db-indexer consumer (OCW).
//!
//! Tests pre-populate the offchain DB as if `EventCapture::capture_events`
//! had run during block execution — that means writing the per-extrinsic
//! event payload at `key_for_event(block, ext_idx)` and the per-block
//! high-water-mark at `key_for_high_water(block)` — then drive
//! `offchain_worker` and verify the OCW reads, forwards in order, and
//! deletes consumed entries.

use std::borrow::Cow;

use codec::Encode;
use polkadot_sdk::frame_support::traits::Hooks;
use polkadot_sdk::sp_core::offchain::testing::{PendingRequest, TestOffchainExt};
use polkadot_sdk::sp_core::offchain::{OffchainDbExt, OffchainStorage, OffchainWorkerExt};
use polkadot_sdk::sp_runtime::offchain::storage_lock::{StorageLock, Time};
use polkadot_sdk::sp_runtime::offchain::Duration;
use prost::Message;
use sxt_core::prover_db_indexer::{
    key_for_event,
    key_for_high_water,
    BlockEvent,
    CreateEntry,
    EventCapture,
    IncludeRule,
    IncludeRules,
    InsertEntry,
};
use sxt_core::tables::{TableIdentifier, TableNamespace};

use crate::mock::*;
use crate::{proto, IncludeSet, PROVER_DB_URL_KEY};

type StateArc =
    std::sync::Arc<parking_lot::RwLock<polkadot_sdk::sp_core::offchain::testing::OffchainState>>;

const MOCK_URL: &str = "http://127.0.0.1:9999";

fn encode_url() -> Vec<u8> {
    codec::Encode::encode(&MOCK_URL.as_bytes().to_vec())
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

fn setup_with_url() -> (polkadot_sdk::sp_io::TestExternalities, StateArc) {
    let mut ext = new_test_ext();
    let (offchain, state) = TestOffchainExt::new();
    ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
    ext.register_extension(OffchainDbExt::new(offchain));
    state
        .write()
        .persistent_storage
        .set(b"", PROVER_DB_URL_KEY, &encode_url());
    (ext, state)
}

/// Mirror what the producer would write for a block: one event payload
/// at `(block, ext_idx)` and the matching high-water-mark.
fn seed_block_events(state: &StateArc, block: u64, ext_idx: u32, events: Vec<BlockEvent<'static>>) {
    let mut s = state.write();
    s.persistent_storage
        .set(b"", &key_for_event(block, ext_idx), &events.encode());
    s.persistent_storage
        .set(b"", &key_for_high_water(block), &Encode::encode(&ext_idx));
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
fn ocw_forwards_and_deletes_offchain_entry() {
    let (mut ext, state) = setup_with_url();

    let ddl = b"CREATE TABLE PUBLIC.USERS (ID BIGINT NOT NULL)";
    seed_block_events(
        &state,
        1,
        0,
        vec![BlockEvent::Create(CreateEntry {
            ident: Cow::Owned(TableIdentifier::from_str_unchecked("USERS", "PUBLIC")),
            ddl: ddl.to_vec().into(),
        })],
    );

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

    let s = state.read();
    assert!(
        s.persistent_storage.get(&key_for_event(1, 0)).is_none(),
        "consumed event payload should be deleted"
    );
    assert!(
        s.persistent_storage.get(&key_for_high_water(1)).is_none(),
        "high-water-mark should be deleted"
    );
}

#[test]
fn ocw_checkpoints_empty_blocks() {
    let (mut ext, state) = setup_with_url();

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

    seed_block_events(
        &state,
        6,
        0,
        vec![BlockEvent::Drop(Cow::Owned(
            TableIdentifier::from_str_unchecked("OLD", "NS"),
        ))],
    );

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

    seed_block_events(
        &state,
        1,
        0,
        vec![BlockEvent::Drop(Cow::Owned(
            TableIdentifier::from_str_unchecked("T1", "NS"),
        ))],
    );
    seed_block_events(
        &state,
        2,
        0,
        vec![BlockEvent::Create(CreateEntry {
            ident: Cow::Owned(TableIdentifier::from_str_unchecked("T2", "NS")),
            ddl: b"CREATE TABLE NS.T2 (X INT NOT NULL)".to_vec().into(),
        })],
    );

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

    let s = state.read();
    assert!(s.persistent_storage.get(&key_for_event(1, 0)).is_none());
    assert!(s.persistent_storage.get(&key_for_high_water(1)).is_none());
    assert!(s.persistent_storage.get(&key_for_event(2, 0)).is_none());
    assert!(s.persistent_storage.get(&key_for_high_water(2)).is_none());
}

#[test]
fn ocw_walks_multiple_extrinsics_in_one_block() {
    let (mut ext, state) = setup_with_url();

    // Two extrinsics in block 1 fire captures: ext 1 and ext 3. Ext 2
    // didn't (a sparse block). hwm should be 3; the OCW probes 0..=3
    // and finds payloads at 1 and 3.
    let mut s = state.write();
    s.persistent_storage.set(
        b"",
        &key_for_event(1, 1),
        &vec![BlockEvent::Drop(Cow::Owned(
            TableIdentifier::from_str_unchecked("T1", "NS"),
        ))]
        .encode(),
    );
    s.persistent_storage.set(
        b"",
        &key_for_event(1, 3),
        &vec![BlockEvent::Insert(InsertEntry {
            table: Cow::Owned(TableIdentifier::from_str_unchecked("T2", "NS")),
            data: b"row-data".to_vec().into(),
        })]
        .encode(),
    );
    s.persistent_storage
        .set(b"", &key_for_high_water(1), &Encode::encode(&3u32));
    drop(s);

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

    let s = state.read();
    assert!(s.persistent_storage.get(&key_for_event(1, 1)).is_none());
    assert!(s.persistent_storage.get(&key_for_event(1, 3)).is_none());
    assert!(s.persistent_storage.get(&key_for_high_water(1)).is_none());
}

// ─── Include-set tests ──────────────────────────────────────────────────

/// Helper: build a Drop event for `(name, namespace)`.
fn drop_event(name: &str, namespace: &str) -> BlockEvent<'static> {
    BlockEvent::Drop(Cow::Owned(TableIdentifier::from_str_unchecked(
        name, namespace,
    )))
}

/// Helper: build a Create event for `(name, namespace)` with arbitrary DDL.
fn create_event(name: &str, namespace: &str) -> BlockEvent<'static> {
    BlockEvent::Create(CreateEntry {
        ident: Cow::Owned(TableIdentifier::from_str_unchecked(name, namespace)),
        ddl: Cow::Owned(b"CREATE TABLE ...".to_vec()),
    })
}

/// Helper: build an Insert event for `(name, namespace)` with arbitrary data.
fn insert_event(name: &str, namespace: &str) -> BlockEvent<'static> {
    BlockEvent::Insert(InsertEntry {
        table: Cow::Owned(TableIdentifier::from_str_unchecked(name, namespace)),
        data: Cow::Owned(b"rows".to_vec()),
    })
}

/// Set up an externalities for exercising the producer side
/// (`capture_events`). No `TestOffchainExt` needed — `offchain_index::set`
/// goes through the runtime overlay into `ext.offchain_db()`, which the
/// tests inspect after calling `persist_offchain_overlay`.
fn capture_ext() -> polkadot_sdk::sp_io::TestExternalities {
    new_test_ext()
}

#[test]
fn set_include_rules_works_for_root_and_emits_event() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let rules = vec![
            IncludeRule::Namespace(TableNamespace::try_from(b"PUBLIC".to_vec()).unwrap()),
            IncludeRule::Table(TableIdentifier::from_str_unchecked("BAR", "FOO")),
        ]
        .try_into()
        .unwrap();

        assert!(ProverDbIndexer::set_include_rules(RuntimeOrigin::root(), rules).is_ok());

        // Storage now holds two rules.
        assert_eq!(IncludeSet::<Test>::get().len(), 2);

        // And an event was deposited.
        let events = System::events();
        assert!(events.iter().any(|er| matches!(
            er.event,
            RuntimeEvent::ProverDbIndexer(crate::Event::IncludeRulesSet { count: 2 }),
        )));
    });
}

#[test]
fn set_include_rules_rejects_non_root() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let signed = RuntimeOrigin::signed(polkadot_sdk::sp_runtime::AccountId32::new([1; 32]));
        let rules = vec![IncludeRule::Namespace(
            TableNamespace::try_from(b"PUBLIC".to_vec()).unwrap(),
        )]
        .try_into()
        .unwrap();
        assert!(ProverDbIndexer::set_include_rules(signed, rules).is_err());
        // Storage unchanged.
        assert!(IncludeSet::<Test>::get().is_empty());
    });
}

#[test]
fn capture_events_writes_everything_when_include_set_is_empty() {
    let mut ext = capture_ext();
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

#[test]
fn capture_events_keeps_only_namespace_matches() {
    let mut ext = capture_ext();
    ext.execute_with(|| {
        System::set_block_number(1);
        polkadot_sdk::frame_system::Pallet::<Test>::set_extrinsic_index(0);

        IncludeSet::<Test>::put(
            IncludeRules::try_from(vec![IncludeRule::Namespace(
                TableNamespace::try_from(b"ALPHA".to_vec()).unwrap(),
            )])
            .unwrap(),
        );

        let events = vec![
            create_event("A", "ALPHA"), // pass
            insert_event("B", "BETA"),  // filter out
            drop_event("C", "ALPHA"),   // pass
        ];
        <ProverDbIndexer as EventCapture>::capture_events(events);
    });
    ext.persist_offchain_overlay();
    let db = ext.offchain_db();
    let stored = db.get(&key_for_event(1, 0)).unwrap();
    let decoded: Vec<BlockEvent<'static>> = codec::Decode::decode(&mut &stored[..]).unwrap();
    assert_eq!(decoded.len(), 2);
}

#[test]
fn capture_events_keeps_only_specific_table_matches() {
    let mut ext = capture_ext();
    ext.execute_with(|| {
        System::set_block_number(1);
        polkadot_sdk::frame_system::Pallet::<Test>::set_extrinsic_index(0);

        IncludeSet::<Test>::put(
            IncludeRules::try_from(vec![IncludeRule::Table(
                TableIdentifier::from_str_unchecked("B", "BETA"),
            )])
            .unwrap(),
        );

        let events = vec![
            create_event("A", "ALPHA"), // filter out
            insert_event("B", "BETA"),  // pass
            drop_event("C", "BETA"),    // filter out: namespace match but rule is table-scoped
        ];
        <ProverDbIndexer as EventCapture>::capture_events(events);
    });
    ext.persist_offchain_overlay();
    let db = ext.offchain_db();
    let stored = db.get(&key_for_event(1, 0)).unwrap();
    let decoded: Vec<BlockEvent<'static>> = codec::Decode::decode(&mut &stored[..]).unwrap();
    assert_eq!(decoded.len(), 1);
    assert!(matches!(decoded[0], BlockEvent::Insert(_)));
}

#[test]
fn capture_events_writes_nothing_when_no_event_matches() {
    let mut ext = capture_ext();
    ext.execute_with(|| {
        System::set_block_number(1);
        polkadot_sdk::frame_system::Pallet::<Test>::set_extrinsic_index(0);

        IncludeSet::<Test>::put(
            IncludeRules::try_from(vec![IncludeRule::Namespace(
                TableNamespace::try_from(b"NOT_PRESENT".to_vec()).unwrap(),
            )])
            .unwrap(),
        );

        let events = vec![create_event("A", "ALPHA"), insert_event("B", "BETA")];
        <ProverDbIndexer as EventCapture>::capture_events(events);
    });
    ext.persist_offchain_overlay();
    let db = ext.offchain_db();
    assert!(db.get(&key_for_event(1, 0)).is_none());
    assert!(db.get(&key_for_high_water(1)).is_none());
}
