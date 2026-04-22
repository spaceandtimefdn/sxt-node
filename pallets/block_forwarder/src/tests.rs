//! Integration tests for the block forwarder pallet.
//!
//! The OCW always asks the server for its last checkpoint (no local cursor).
//! Tests pre-populate the offchain DB as if on_finalize had written entries,
//! then verify the OCW reads, forwards in order, and deletes consumed entries.

use codec::Encode;
use polkadot_sdk::frame_support::traits::Hooks;
use polkadot_sdk::sp_core::offchain::testing::{PendingRequest, TestOffchainExt};
use polkadot_sdk::sp_core::offchain::{OffchainDbExt, OffchainStorage, OffchainWorkerExt};
use polkadot_sdk::sp_runtime::BoundedVec;
use prost::Message;
use sxt_core::tables::TableIdentifier;

use crate::mock::*;
use crate::{offchain_index, proto, INDEXER_URL_KEY};

type StateArc =
    std::sync::Arc<parking_lot::RwLock<polkadot_sdk::sp_core::offchain::testing::OffchainState>>;

const MOCK_URL: &str = "http://127.0.0.1:9999";

fn encode_url() -> Vec<u8> {
    codec::Encode::encode(&MOCK_URL.as_bytes().to_vec())
}

fn table_id(namespace: &str, name: &str) -> TableIdentifier {
    TableIdentifier {
        namespace: BoundedVec::try_from(namespace.as_bytes().to_vec()).unwrap(),
        name: BoundedVec::try_from(name.as_bytes().to_vec()).unwrap(),
    }
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
        .set(b"", INDEXER_URL_KEY, &encode_url());
    (ext, state)
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
        BlockForwarder::offchain_worker(1);
    });
}

#[test]
fn ocw_forwards_and_deletes_offchain_entry() {
    let (mut ext, state) = setup_with_url();

    let ddl = b"CREATE TABLE PUBLIC.USERS (ID BIGINT NOT NULL)";
    let index = offchain_index::BlockIndex {
        events: vec![offchain_index::BlockEvent::Create(
            offchain_index::CreateEntry {
                ident: table_id("PUBLIC", "USERS"),
                ddl: ddl.to_vec(),
            },
        )],
    };
    let key = offchain_index::key_for_block(1);
    state
        .write()
        .persistent_storage
        .set(b"", &key, &index.encode());

    {
        let mut s = state.write();
        // 1. get_last_checkpoint → server at block 0
        s.expect_request(expected_request(
            "/v1/get_last_checkpoint",
            vec![],
            no_checkpoint_response(),
        ));
        // 2. create_table for block 1
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
        // 3. checkpoint(1)
        s.expect_request(expected_request(
            "/v1/checkpoint",
            proto::CheckpointRequest { sequence_number: 1 }.encode_to_vec(),
            vec![],
        ));
    }

    ext.execute_with(|| {
        System::set_block_number(1);
        BlockForwarder::offchain_worker(1);
    });

    // Verify offchain entry was deleted.
    let s = state.read();
    assert!(
        s.persistent_storage.get(&key).is_none(),
        "consumed entry should be deleted"
    );
}

#[test]
fn ocw_checkpoints_empty_blocks() {
    let (mut ext, state) = setup_with_url();

    {
        let mut s = state.write();
        // Server at block 0.
        s.expect_request(expected_request(
            "/v1/get_last_checkpoint",
            vec![],
            no_checkpoint_response(),
        ));
        // Empty block 1 → just checkpoint.
        s.expect_request(expected_request(
            "/v1/checkpoint",
            proto::CheckpointRequest { sequence_number: 1 }.encode_to_vec(),
            vec![],
        ));
    }

    ext.execute_with(|| {
        System::set_block_number(1);
        BlockForwarder::offchain_worker(1);
    });
}

#[test]
fn ocw_resumes_from_server_checkpoint() {
    let (mut ext, state) = setup_with_url();

    // Server already at block 5. Block 6 has a drop event.
    let index = offchain_index::BlockIndex {
        events: vec![offchain_index::BlockEvent::Drop(table_id("NS", "OLD"))],
    };
    state
        .write()
        .persistent_storage
        .set(b"", &offchain_index::key_for_block(6), &index.encode());

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
        BlockForwarder::offchain_worker(6);
    });
}

#[test]
fn ocw_processes_multiple_blocks_in_order() {
    let (mut ext, state) = setup_with_url();

    // Block 1: drop T1. Block 2: create T2. Block 3: empty.
    state.write().persistent_storage.set(
        b"",
        &offchain_index::key_for_block(1),
        &offchain_index::BlockIndex {
            events: vec![offchain_index::BlockEvent::Drop(table_id("NS", "T1"))],
        }
        .encode(),
    );
    state.write().persistent_storage.set(
        b"",
        &offchain_index::key_for_block(2),
        &offchain_index::BlockIndex {
            events: vec![offchain_index::BlockEvent::Create(
                offchain_index::CreateEntry {
                    ident: table_id("NS", "T2"),
                    ddl: b"CREATE TABLE NS.T2 (X INT NOT NULL)".to_vec(),
                },
            )],
        }
        .encode(),
    );

    {
        let mut s = state.write();
        // get_last_checkpoint → server at 0
        s.expect_request(expected_request(
            "/v1/get_last_checkpoint",
            vec![],
            no_checkpoint_response(),
        ));
        // Block 1: drop + checkpoint
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
        // Block 2: create + checkpoint
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
        // Block 3: empty, just checkpoint
        s.expect_request(expected_request(
            "/v1/checkpoint",
            proto::CheckpointRequest { sequence_number: 3 }.encode_to_vec(),
            vec![],
        ));
    }

    ext.execute_with(|| {
        System::set_block_number(3);
        BlockForwarder::offchain_worker(3);
    });

    // Both consumed entries should be deleted.
    let s = state.read();
    assert!(s
        .persistent_storage
        .get(&offchain_index::key_for_block(1))
        .is_none());
    assert!(s
        .persistent_storage
        .get(&offchain_index::key_for_block(2))
        .is_none());
}
