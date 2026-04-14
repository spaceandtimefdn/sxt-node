//! Integration tests for the offchain indexing pallet.

use codec::Encode;
use frame_support::traits::Hooks;
use prost::Message;
use sp_core::offchain::testing::{PendingRequest, TestOffchainExt};
use sp_core::offchain::{OffchainDbExt, OffchainStorage, OffchainWorkerExt};
use sp_runtime::BoundedVec;
use sxt_core::tables::{TableIdentifier, TableType};

use crate::mock::*;
use crate::proto;
use crate::{INDEXER_URL_KEY, LAST_FORWARDED_BLOCK_KEY};

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

type StateArc = std::sync::Arc<parking_lot::RwLock<sp_core::offchain::testing::OffchainState>>;

fn setup_with_url() -> (sp_io::TestExternalities, StateArc) {
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

// ─── Tests ──────────────────────────────────────────────────────────────────

#[test]
fn ocw_skips_when_not_configured() {
    let mut ext = new_test_ext();
    let (offchain, _) = TestOffchainExt::new();
    ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
    ext.register_extension(OffchainDbExt::new(offchain));
    ext.execute_with(|| {
        System::set_block_number(1);
        OffchainIndexing::offchain_worker(1);
    });
}

#[test]
fn ocw_skips_already_forwarded_block() {
    let (mut ext, state) = setup_with_url();
    state.write().persistent_storage.set(
        b"",
        LAST_FORWARDED_BLOCK_KEY,
        &codec::Encode::encode(&5u64),
    );
    ext.execute_with(|| {
        System::set_block_number(5);
        OffchainIndexing::offchain_worker(5);
    });
}

#[test]
fn ocw_checkpoints_empty_block_when_cursor_set() {
    // Simplest case: cursor=0, block=1, no events → just checkpoint(1)
    let (mut ext, state) = setup_with_url();
    state.write().persistent_storage.set(
        b"",
        LAST_FORWARDED_BLOCK_KEY,
        &codec::Encode::encode(&0u64),
    );
    {
        let mut s = state.write();
        s.expect_request(expected_request(
            "/v1/checkpoint",
            proto::CheckpointRequest { sequence_number: 1 }.encode_to_vec(),
            vec![],
        ));
    }
    ext.execute_with(|| {
        System::set_block_number(1);
        OffchainIndexing::offchain_worker(1);
    });
    let s = state.read();
    let cursor_bytes = s.persistent_storage.get(LAST_FORWARDED_BLOCK_KEY).unwrap();
    let cursor: u64 = codec::Decode::decode(&mut &cursor_bytes[..]).unwrap();
    assert_eq!(cursor, 1);
}

#[test]
fn ocw_forwards_table_dropped_event() {
    let (mut ext, state) = setup_with_url();
    state.write().persistent_storage.set(
        b"",
        LAST_FORWARDED_BLOCK_KEY,
        &codec::Encode::encode(&0u64),
    );
    {
        let mut s = state.write();
        // Expect: drop_table, then checkpoint
        s.expect_request(expected_request(
            "/v1/drop_table",
            proto::DropTableRequest {
                sequence_number: 1,
                table_name: "PUBLIC.OLD_TABLE".into(),
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
        frame_system::Pallet::<Test>::deposit_event(RuntimeEvent::Tables(
            pallet_tables::Event::<Test>::TableDropped(
                None,
                TableType::Community,
                table_id("PUBLIC", "OLD_TABLE"),
            ),
        ));
        OffchainIndexing::offchain_worker(1);
    });
}

#[test]
fn ocw_forwards_schema_updated_event() {
    let (mut ext, state) = setup_with_url();
    state.write().persistent_storage.set(
        b"",
        LAST_FORWARDED_BLOCK_KEY,
        &codec::Encode::encode(&0u64),
    );

    let ddl = b"CREATE TABLE PUBLIC.USERS (ID BIGINT NOT NULL, NAME VARCHAR NOT NULL)";
    let expected_schema = crate::translate::ddl_to_arrow_ipc_schema(ddl).unwrap();

    {
        let mut s = state.write();
        s.expect_request(expected_request(
            "/v1/create_table",
            proto::CreateTableRequest {
                sequence_number: 1,
                table_name: "PUBLIC.USERS".into(),
                arrow_schema: expected_schema,
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
        let update = pallet_tables::UpdateTable {
            ident: table_id("PUBLIC", "USERS"),
            create_statement: BoundedVec::try_from(ddl.to_vec()).unwrap(),
            table_type: TableType::Community,
            commitment: pallet_tables::CommitmentCreationCmd::Empty(Default::default()),
            source: Default::default(),
        };
        frame_system::Pallet::<Test>::deposit_event(RuntimeEvent::Tables(
            pallet_tables::Event::<Test>::SchemaUpdated(
                None,
                BoundedVec::try_from(vec![update]).unwrap(),
            ),
        ));
        OffchainIndexing::offchain_worker(1);
    });
}

// ─── Data path unit tests (QuorumReached pipeline) ──────────────────────────
//
// pallet_indexing has heavy deps (staking, session, balances) that make adding
// it to the mock runtime impractical. Instead we test the data conversion
// pipeline directly — the same code path that QuorumReached events exercise.

/// OnChainTable → postcard → on_chain_tables_to_arrow_ipc → Arrow IPC.
#[test]
fn on_chain_table_round_trips_through_arrow_ipc() {
    use std::io::Cursor;

    use arrow::ipc::reader::StreamReader;
    use on_chain_table::{OnChainColumn, OnChainTable};
    use sqlparser::ast::Ident;

    let table = OnChainTable::try_from_iter([
        (Ident::new("ID"), OnChainColumn::BigInt(vec![1, 2, 3])),
        (
            Ident::new("NAME"),
            OnChainColumn::VarChar(vec!["alice".into(), "bob".into(), "charlie".into()]),
        ),
    ])
    .unwrap();

    let postcard_bytes = postcard::to_allocvec(&table).unwrap();
    let ipc_bytes = crate::translate::on_chain_tables_to_arrow_ipc(&[postcard_bytes]).unwrap();

    let reader = StreamReader::try_new(Cursor::new(&ipc_bytes), None).unwrap();
    let schema = reader.schema();
    let col_names: Vec<&str> = schema.fields().iter().map(|f| f.name().as_str()).collect();
    assert_eq!(col_names, vec!["ID", "NAME"]);

    let batches: Vec<_> = reader.into_iter().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].num_rows(), 3);
    assert_eq!(batches[0].num_columns(), 2);
}

/// DDL-derived schema must match data-derived schema for the same table structure.
#[test]
fn ddl_schema_matches_data_schema() {
    use std::io::Cursor;

    use arrow::ipc::reader::StreamReader;
    use on_chain_table::{OnChainColumn, OnChainTable};
    use sqlparser::ast::Ident;

    let ddl = b"CREATE TABLE test.t (ID BIGINT NOT NULL, NAME VARCHAR NOT NULL)";
    let schema_ipc = crate::translate::ddl_to_arrow_ipc_schema(ddl).unwrap();
    let ddl_schema = StreamReader::try_new(Cursor::new(&schema_ipc), None)
        .unwrap()
        .schema()
        .clone();

    let table = OnChainTable::try_from_iter([
        (Ident::new("ID"), OnChainColumn::BigInt(vec![42])),
        (Ident::new("NAME"), OnChainColumn::VarChar(vec!["x".into()])),
    ])
    .unwrap();
    let data_ipc =
        crate::translate::on_chain_tables_to_arrow_ipc(&[postcard::to_allocvec(&table).unwrap()])
            .unwrap();
    let data_schema = StreamReader::try_new(Cursor::new(&data_ipc), None)
        .unwrap()
        .schema()
        .clone();

    assert_eq!(ddl_schema.fields(), data_schema.fields());
}

/// Multiple OnChainTable blobs fold into one IPC stream with multiple batches.
#[test]
fn multiple_on_chain_tables_fold_into_one_ipc_stream() {
    use std::io::Cursor;

    use arrow::ipc::reader::StreamReader;
    use on_chain_table::{OnChainColumn, OnChainTable};
    use sqlparser::ast::Ident;

    let blob1 = postcard::to_allocvec(
        &OnChainTable::try_from_iter([(Ident::new("VAL"), OnChainColumn::Int(vec![1, 2]))])
            .unwrap(),
    )
    .unwrap();
    let blob2 = postcard::to_allocvec(
        &OnChainTable::try_from_iter([(Ident::new("VAL"), OnChainColumn::Int(vec![3, 4, 5]))])
            .unwrap(),
    )
    .unwrap();

    let ipc = crate::translate::on_chain_tables_to_arrow_ipc(&[blob1, blob2]).unwrap();
    let batches: Vec<_> = StreamReader::try_new(Cursor::new(&ipc), None)
        .unwrap()
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert_eq!(batches.len(), 2);
    assert_eq!(batches[0].num_rows(), 2);
    assert_eq!(batches[1].num_rows(), 3);
}
