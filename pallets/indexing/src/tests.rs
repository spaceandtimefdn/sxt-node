use alloc::boxed::Box;
use std::collections::{HashMap, HashSet};
use std::convert::Into;
use std::io::Cursor;
use std::sync::Arc;

use arrow::array::{ArrayRef, Int32Array, Int64Array, RecordBatch};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::ipc::writer::StreamWriter;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::__private::RuntimeDebug;
use frame_support::dispatch::DispatchResult;
use frame_support::pallet_prelude::TypeInfo;
use frame_support::traits::{Get, Hooks};
use frame_support::{assert_err, assert_noop, assert_ok};
use frame_system::{ensure_signed, RawOrigin};
use native_api::{Api, NativeApi};
use on_chain_table::proptest::{on_chain_table, ProofOfSqlSchema};
use on_chain_table::{IndexSet, OnChainTable};
use pallet_tables::{CommitmentCreationCmd, UpdateTable};
use proof_of_sql::base::database::ColumnType;
use proof_of_sql_commitment_map::CommitmentSchemeFlags;
use proptest::prelude::*;
use proptest::sample::SizeRange;
use sp_core::Hasher;
use sp_runtime::BoundedVec;
use sqlparser::ast::Ident;
use sxt_core::indexing::{SubmittersByScope, ID_LEN, MAX_SUBMITTERS};
use sxt_core::permissions::{IndexingPalletPermission, PermissionLevel, PermissionList};
use sxt_core::tables::{
    CommitmentScheme,
    CreateStatement,
    InsertQuorumSize,
    QuorumScope,
    SourceAndMode,
    TableIdentifier,
    TableName,
    TableNamespace,
    TableType,
};

use crate::mock::*;
use crate::{build_inner_batch_id, BatchId, Config, Event, RowData};

/// Used as a convenience wrapper for data we need to submit
#[derive(Clone, Encode, Decode, Eq, PartialEq, TypeInfo, MaxEncodedLen, Hash)]
struct TestSubmission {
    table: TableIdentifier,
    batch_id: BatchId,
    data: RowData,
}

impl core::fmt::Debug for TestSubmission {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
        fmt.debug_struct("TestSubmission")
            .field("table", &String::try_from(&self.table).unwrap())
            .field("batch_id", &hex::encode(&self.batch_id))
            .field("data", &hex::encode(&self.data))
            .finish()
    }
}

/// Helper function to streamline data submission
fn submit_test_data(signer: RuntimeOrigin, submission: TestSubmission) -> DispatchResult {
    Indexing::submit_data(
        signer.clone(),
        submission.table.clone(),
        submission.batch_id.clone(),
        submission.data.clone(),
    )
}

fn row_data_with_count(rows: i32) -> RowData {
    let schema = Arc::new(Schema::new(vec![Field::new(
        "int_column",
        DataType::Int32,
        false,
    )]));

    let int_data = Arc::new(Int32Array::from((0..rows).collect::<Vec<i32>>())) as ArrayRef;

    let batch = RecordBatch::try_new(schema.clone(), vec![int_data]).unwrap();

    record_batch_to_row_data(batch)
}

fn row_data() -> RowData {
    row_data_with_count(4)
}

fn diff_row_data() -> RowData {
    let schema = Arc::new(Schema::new(vec![Field::new(
        "int_column",
        DataType::Int32,
        false,
    )]));

    let int_data = Arc::new(Int32Array::from(vec![2, 4, 6, 8])) as ArrayRef;

    let batch = RecordBatch::try_new(schema.clone(), vec![int_data]).unwrap();

    record_batch_to_row_data(batch)
}

fn record_batch_to_row_data(batch: RecordBatch) -> RowData {
    let buffer: Vec<u8> = Vec::new();
    let mut cursor = Cursor::new(buffer);

    let mut writer = StreamWriter::try_new(&mut cursor, batch.schema().as_ref()).unwrap();

    writer.write(&batch).unwrap();
    writer.finish().unwrap();

    let data = writer.into_inner().unwrap().clone();
    let data = data.into_inner().clone();

    RowData::try_from(data).unwrap()
}

fn sample_table_definition() -> (TableIdentifier, CreateStatement) {
    let table_id = TableIdentifier {
        namespace: TableNamespace::try_from(b"TEST_NAMESPACE".to_owned().to_vec()).unwrap(),
        name: TableName::try_from(b"TEST_TABLE".to_owned().to_vec()).unwrap(),
    };
    let create_statement = CreateStatement::try_from(
        b"CREATE TABLE TEST_NAMESPACE.TEST_TABLE (int_column INT NOT NULL)"
            .to_owned()
            .to_vec(),
    )
    .unwrap();

    assert_ok!(Tables::create_namespace(
        RuntimeOrigin::root(),
        table_id.namespace.clone(),
        0,
        b"CREATE SCHEMA IF NOT EXISTS TEST_NAMESPACE"
            .to_vec()
            .try_into()
            .unwrap(),
        TableType::CoreBlockchain,
        sxt_core::tables::Source::Ethereum,
    ));

    (table_id, create_statement)
}

/// Returns a `Strategy` for row data compatible with [`sample_table_definition`].
fn row_data_for_sample_table<NR>(num_rows: NR) -> impl Strategy<Value = RowData>
where
    NR: Strategy<Value = usize>,
{
    let schema =
        ProofOfSqlSchema::try_from_iter([(Ident::new("INT_COLUMN"), ColumnType::Int)]).unwrap();

    on_chain_table(Just(schema), num_rows).prop_map(|on_chain_table| {
        let record_batch = on_chain_table.into();
        record_batch_to_row_data(record_batch)
    })
}

/// Returns a `Strategy` for test submissions compatible with [`sample_table_definition`].
fn submission_for_sample_table<NR, BI>(
    num_rows: NR,
    batch_id: BI,
) -> impl Strategy<Value = TestSubmission>
where
    NR: Strategy<Value = usize>,
    BI: Strategy<Value = BatchId>,
{
    (row_data_for_sample_table(num_rows), batch_id).prop_map(|(data, batch_id)| {
        let table = TableIdentifier::from_str_unchecked("TEST_TABLE", "TEST_NAMESPACE");
        TestSubmission {
            table,
            batch_id,
            data,
        }
    })
}

/// Returns a `Strategy` for `BatchId`s.
fn batch_id_strategy() -> impl Strategy<Value = BatchId> {
    proptest::collection::vec(any::<u8>(), 1..ID_LEN as usize)
        .prop_map(|batch_id_bytes| batch_id_bytes.try_into().unwrap())
}

/// Returns a `Strategy` for a set of test submissions for [`sample_table_definition`].
fn submissions_for_sample_table<NS, NR, BI>(
    num_submissions: NS,
    num_rows_per_submission: NR,
    batch_id: BI,
) -> impl Strategy<Value = HashSet<TestSubmission>>
where
    NS: Into<SizeRange> + Clone,
    NR: Strategy<Value = usize> + Clone,
    BI: Strategy<Value = BatchId>,
{
    proptest::collection::hash_set(
        submission_for_sample_table(num_rows_per_submission, batch_id),
        num_submissions,
    )
}

/// Returns a `Strategy` for a mapping of `BatchId`s to a set of test submissions for [`sample_table_definition`].
fn submissions_for_sample_table_by_batch_id<NB, NS, NR, BI>(
    num_batches: NB,
    num_submissions_per_batch: NS,
    num_rows_per_submission: NR,
    batch_id: BI,
) -> impl Strategy<Value = HashMap<BatchId, HashSet<TestSubmission>>>
where
    NB: Into<SizeRange>,
    NS: Into<SizeRange> + Clone,
    NR: Strategy<Value = usize> + Clone,
    BI: Strategy<Value = BatchId>,
{
    proptest::collection::hash_set(batch_id, num_batches)
        .prop_flat_map(move |batch_ids| {
            let num_submissions_per_batch = num_submissions_per_batch.clone();
            let num_rows_per_submission = num_rows_per_submission.clone();
            batch_ids
                .into_iter()
                .map(move |batch_id| {
                    (
                        Just(batch_id.clone()),
                        submissions_for_sample_table(
                            num_submissions_per_batch.clone(),
                            num_rows_per_submission.clone(),
                            Just(batch_id),
                        ),
                    )
                })
                .collect::<Vec<_>>()
        })
        .prop_map(HashMap::from_iter)
}

fn empty_row_data() -> RowData {
    let schema = Arc::new(Schema::new(vec![Field::new(
        "int_column",
        DataType::Int32,
        false,
    )]));

    let empty_batch = RecordBatch::new_empty(schema.clone());

    record_batch_to_row_data(empty_batch)
}

fn row_data_w_block_number() -> RowData {
    let schema = Arc::new(Schema::new(vec![
        Field::new("int_column", DataType::Int32, false),
        Field::new("block_number", DataType::Int64, false),
    ]));

    let int_data = Arc::new(Int32Array::from(vec![1, 2, 3, 4])) as ArrayRef;
    let block_data = Arc::new(Int64Array::from(vec![100, 101, 102, 12345])) as ArrayRef;

    let batch = RecordBatch::try_new(schema.clone(), vec![int_data, block_data]).unwrap();

    record_batch_to_row_data(batch)
}

fn sample_table_definition_with_block_number() -> (TableIdentifier, CreateStatement) {
    let table_id = TableIdentifier {
        namespace: TableNamespace::try_from(b"TEST_NAMESPACE".to_vec()).unwrap(),
        name: TableName::try_from(b"TEST_TABLE".to_vec()).unwrap(),
    };

    // Matches the schema used in `row_data_w_block_number`
    let create_statement = CreateStatement::try_from(
        b"CREATE TABLE TEST_NAMESPACE.TEST_TABLE (
            int_column INT NOT NULL,
            block_number BIGINT NOT NULL
        )"
        .to_vec(),
    )
    .unwrap();

    assert_ok!(Tables::create_namespace(
        RuntimeOrigin::root(),
        table_id.namespace.clone(),
        0,
        b"CREATE SCHEMA IF NOT EXISTS TEST_NAMESPACE"
            .to_vec()
            .try_into()
            .unwrap(),
        TableType::CoreBlockchain,
        sxt_core::tables::Source::Ethereum,
    ));

    (table_id, create_statement)
}

fn hash_row_data_with_block_number<T: frame_system::Config>(
    row_data: &RowData,
    block_number: Option<u64>,
) -> T::Hash {
    let mut input = row_data.encode();
    input.extend(block_number.encode());
    <T::Hashing as Hasher>::hash(&input)
}

#[test]
fn inserting_data_succeeds_when_data_is_good() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (table_id, test_create) = sample_table_definition();

        let request = UpdateTable {
            ident: table_id.clone(),
            create_statement: test_create,
            table_type: TableType::Testing(InsertQuorumSize {
                public: Some(1),
                privileged: None,
            }),
            commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags {
                hyper_kzg: true,
                dynamic_dory: true,
            }),
            source: sxt_core::tables::Source::Ethereum,
        };
        Tables::create_tables(RuntimeOrigin::root(), vec![request].try_into().unwrap()).unwrap();

        let signer = RuntimeOrigin::signed(sp_runtime::AccountId32::new([1; 32]));
        let who = ensure_signed(signer.clone()).unwrap();
        let permissions = PermissionList::try_from(vec![PermissionLevel::IndexingPallet(
            IndexingPalletPermission::SubmitDataForPublicQuorum,
        )])
        .unwrap();
        pallet_permissions::Permissions::<Test>::insert(&who, permissions.clone());

        let test_batch = BatchId::try_from(b"test_batch".to_vec()).unwrap();
        let test_data = row_data();

        assert_ok!(Indexing::submit_data(
            signer.clone(),
            table_id.clone(),
            test_batch.clone(),
            test_data.clone(),
        ),);

        let hash = hash_row_data_with_block_number::<Test>(&test_data, None);

        let internal_batch_id = build_inner_batch_id::<Test, Api>(&test_batch, &table_id);
        // Verify that the submission was stored as expected
        // and the hash was generated from the submitted data
        assert_eq!(
            Indexing::submissions_v1((internal_batch_id.clone(), QuorumScope::Public, who)),
            Some(hash)
        );
    })
}

#[test]
fn data_submission_fails_if_no_permissions() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let (test_identifier, _) = sample_table_definition();

        let test_batch = BatchId::try_from(b"test_batch".to_vec()).unwrap();
        let test_data = RowData::try_from(b"some arbitrary row data".to_vec()).unwrap();

        // Create a non permissioned signer
        let account = sp_runtime::AccountId32::new([1; 32]);
        let signer = RuntimeOrigin::signed(account.clone());
        assert_err!(
            Indexing::submit_data(
                signer.clone(),
                test_identifier.clone(),
                test_batch.clone(),
                test_data.clone(),
            ),
            crate::Error::<Test, Api>::UnauthorizedSubmitter,
        );

        let _ = <<Test as frame_system::Config>::Hashing as Hasher>::hash(&test_data);

        // Verify that the submission was not stored
        let internal_batch_id = build_inner_batch_id::<Test, Api>(&test_batch, &test_identifier);
        let submitters_count =
            crate::SubmissionsV1::<Test, Api>::iter_prefix((internal_batch_id.clone(),)).count();
        assert_eq!(submitters_count, 0);
    })
}

/// This test checks that a quorum is reached, final data is recorded, and extra data is removed
/// after the required number of submissions are sent
#[test]
fn data_is_decided_on_after_required_submissions() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (table_id, create_statement) = sample_table_definition();
        Tables::create_tables(
            RuntimeOrigin::root(),
            vec![UpdateTable {
                ident: table_id.clone(),
                create_statement,
                table_type: TableType::CoreBlockchain,
                commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags {
                    hyper_kzg: true,
                    dynamic_dory: true,
                }),
                source: sxt_core::tables::Source::Ethereum,
            }]
            .try_into()
            .unwrap(),
        )
        .unwrap();

        let test_submission = TestSubmission {
            table: table_id.clone(),
            batch_id: BatchId::try_from(b"test_batch".to_vec()).unwrap(),
            data: row_data(),
        };
        let test_data_hash = hash_row_data_with_block_number::<Test>(&test_submission.data, None);

        // Add permissions for the test accounts
        let permissions = PermissionList::try_from(vec![PermissionLevel::IndexingPallet(
            IndexingPalletPermission::SubmitDataForPublicQuorum,
        )])
        .unwrap();
        for id in 0..5 {
            let who = ensure_signed(RuntimeOrigin::signed(sp_runtime::AccountId32::new(
                [id; 32],
            )))
            .unwrap();
            pallet_permissions::Permissions::<Test>::insert(who.clone(), permissions.clone());
        }

        // Submit 4 entries with 4 different accounts
        assert_ok!(submit_test_data(
            RuntimeOrigin::signed(sp_runtime::AccountId32::new([1; 32])),
            test_submission.clone()
        ));
        assert_ok!(submit_test_data(
            RuntimeOrigin::signed(sp_runtime::AccountId32::new([2; 32])),
            test_submission.clone()
        ));
        assert_ok!(submit_test_data(
            RuntimeOrigin::signed(sp_runtime::AccountId32::new([3; 32])),
            test_submission.clone()
        ));

        // We haven't reached enough submissions yet, so this should not be decided on
        assert!(Indexing::final_data(test_submission.batch_id.clone()).is_none());

        // Send the final required submission
        assert_ok!(submit_test_data(
            RuntimeOrigin::signed(sp_runtime::AccountId32::new([4; 32])),
            test_submission.clone()
        ));
        let internal_batch_id =
            build_inner_batch_id::<Test, Api>(&test_submission.batch_id, &table_id);

        // Now that we have 4 submissions, verify that the data was decided on
        let maybe_final_data = Indexing::final_data(&internal_batch_id);
        assert!(maybe_final_data.is_some());

        let fd = maybe_final_data.unwrap();
        assert_eq!(fd.data_hash, test_data_hash);
        assert_eq!(fd.table, test_submission.table);
        assert_eq!(fd.quorum_scope, QuorumScope::Public);

        // Verify that the old data was successfully removed for this batch
        let submitters_count =
            crate::SubmissionsV1::<Test, Api>::iter_prefix((internal_batch_id.clone(),)).count();
        assert_eq!(submitters_count, 0);
    })
}

/// This test aims to verify that the quorum is reached on the 'correct' data
/// even if there are mismatched submissions
#[test]
fn correct_data_is_decided_on_after_required_submissions() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let (table_id, create_statement) = sample_table_definition();
        Tables::create_tables(
            RuntimeOrigin::root(),
            vec![UpdateTable {
                ident: table_id.clone(),
                create_statement,
                table_type: TableType::CoreBlockchain,
                commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags {
                    hyper_kzg: false,
                    dynamic_dory: true,
                }),
                source: sxt_core::tables::Source::Ethereum,
            }]
            .try_into()
            .unwrap(),
        )
        .unwrap();

        // Add permissions for the test accounts
        for id in 1..6 {
            let who = ensure_signed(RuntimeOrigin::signed(sp_runtime::AccountId32::new(
                [id; 32],
            )))
            .unwrap();
            let permissions = PermissionList::try_from(vec![PermissionLevel::IndexingPallet(
                IndexingPalletPermission::SubmitDataForPublicQuorum,
            )])
            .unwrap();
            pallet_permissions::Permissions::<Test>::insert(who, permissions.clone());
        }

        let test_batch_id = BatchId::try_from(b"test_batch".to_vec()).unwrap();
        let internal_batch_id = build_inner_batch_id::<Test, Api>(&test_batch_id, &table_id);

        let test_submission = TestSubmission {
            table: table_id.clone(),
            batch_id: test_batch_id.clone(),
            data: row_data(),
        };
        let data_hash = hash_row_data_with_block_number::<Test>(&test_submission.data, None);

        // Submit 4 entries with 4 different accounts
        assert_ok!(submit_test_data(
            RuntimeOrigin::signed(sp_runtime::AccountId32::new([1; 32])),
            test_submission.clone()
        ));
        assert_ok!(submit_test_data(
            RuntimeOrigin::signed(sp_runtime::AccountId32::new([2; 32])),
            test_submission.clone()
        ));
        assert_ok!(submit_test_data(
            RuntimeOrigin::signed(sp_runtime::AccountId32::new([3; 32])),
            test_submission.clone()
        ));

        // We haven't reached enough submissions yet, so this should not be decided on
        assert!(Indexing::final_data(&internal_batch_id).is_none());

        // Send a submission that is with different data
        let differing_submission = TestSubmission {
            table: table_id.clone(),
            batch_id: test_batch_id,
            data: diff_row_data(),
        };
        assert_ok!(submit_test_data(
            RuntimeOrigin::signed(sp_runtime::AccountId32::new([4; 32])),
            differing_submission.clone()
        ));

        // This should still not be decided on yet, so double check
        assert!(Indexing::final_data(&internal_batch_id).is_none());

        // Now submit a final matching entry
        assert_ok!(submit_test_data(
            RuntimeOrigin::signed(sp_runtime::AccountId32::new([5; 32])),
            test_submission.clone()
        ));

        // Now that we have 4 submissions, verify that the data was decided on
        let final_data = Indexing::final_data(&internal_batch_id);
        assert!(final_data.is_some());

        // Verify that it matches the originally submitted test data
        assert_eq!(final_data.unwrap().data_hash, data_hash);

        // Verify that the old data was successfully removed for this batch
        let submitters_count =
            crate::SubmissionsV1::<Test, Api>::iter_prefix((internal_batch_id.clone(),)).count();
        assert_eq!(submitters_count, 0);
    })
}

#[test]
fn inserting_data_fails_when_data_is_empty() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let signer = RuntimeOrigin::signed(sp_runtime::AccountId32::new([1; 32]));
        let who = ensure_signed(signer.clone()).unwrap();
        let permissions = PermissionList::try_from(vec![PermissionLevel::IndexingPallet(
            IndexingPalletPermission::SubmitDataForPublicQuorum,
        )])
        .unwrap();
        pallet_permissions::Permissions::<Test>::insert(who, permissions.clone());

        let (test_identifier, create_statement) = sample_table_definition();
        Tables::create_tables(
            RuntimeOrigin::root(),
            vec![UpdateTable {
                ident: test_identifier.clone(),
                create_statement,
                table_type: TableType::CoreBlockchain,
                commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags {
                    hyper_kzg: false,
                    dynamic_dory: true,
                }),
                source: sxt_core::tables::Source::Ethereum,
            }]
            .try_into()
            .unwrap(),
        )
        .unwrap();

        let test_batch = BatchId::try_from(b"test_batch".to_vec()).unwrap();

        // Create an empty data submission to ensure the submission fails
        let test_data = RowData::default();

        assert_err!(
            Indexing::submit_data(signer, test_identifier, test_batch, test_data,),
            crate::Error::<Test, Api>::NoData
        );
    })
}

#[test]
fn inserting_data_fails_when_table_name_is_empty() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let signer = RuntimeOrigin::signed(sp_runtime::AccountId32::new([1; 32]));
        let who = ensure_signed(signer.clone()).unwrap();
        let permissions = PermissionList::try_from(vec![PermissionLevel::IndexingPallet(
            IndexingPalletPermission::SubmitDataForPublicQuorum,
        )])
        .unwrap();
        pallet_permissions::Permissions::<Test>::insert(who, permissions.clone());

        let (table_id, create_statement) = sample_table_definition();
        let test_identifier = TableIdentifier {
            // Create an empty table name
            name: TableName::try_from(b"".to_vec()).unwrap(),
            ..table_id
        };

        let create_statement = CreateStatement::try_from(
            b"CREATE TABLE TEST_NAMESPACE.\"\" (int_column INT NOT NULL)"
                .to_owned()
                .to_vec(),
        )
        .unwrap();

        Tables::create_tables(
            RuntimeOrigin::root(),
            vec![UpdateTable {
                ident: test_identifier.clone(),
                create_statement,
                table_type: TableType::CoreBlockchain,
                commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags {
                    hyper_kzg: false,
                    dynamic_dory: true,
                }),
                source: sxt_core::tables::Source::Ethereum,
            }]
            .try_into()
            .unwrap(),
        )
        .unwrap();

        let test_batch = BatchId::try_from(b"test_batch".to_vec()).unwrap();
        let test_data = RowData::try_from(b"some arbitrary row data".to_vec()).unwrap();

        assert_err!(
            Indexing::submit_data(signer, test_identifier, test_batch, test_data,),
            crate::Error::<Test, Api>::InvalidTable
        );
    })
}

#[test]
fn create_namespace_when_table_namespace_is_empty() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert!(Tables::create_namespace(
            RuntimeOrigin::root(),
            b"".to_vec().try_into().unwrap(),
            0,
            b"CREATE SCHEMA IF NOT EXISTS \"\""
                .to_vec()
                .try_into()
                .unwrap(),
            TableType::CoreBlockchain,
            sxt_core::tables::Source::Ethereum,
        )
        .is_err());
    })
}

#[test]
fn inserting_data_fails_when_batch_id_is_empty() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let signer = RuntimeOrigin::signed(sp_runtime::AccountId32::new([1; 32]));
        let who = ensure_signed(signer.clone()).unwrap();
        let permissions = PermissionList::try_from(vec![PermissionLevel::IndexingPallet(
            IndexingPalletPermission::SubmitDataForPublicQuorum,
        )])
        .unwrap();
        pallet_permissions::Permissions::<Test>::insert(who, permissions.clone());

        let (test_identifier, create_statement) = sample_table_definition();

        Tables::create_tables(
            RuntimeOrigin::root(),
            vec![UpdateTable {
                ident: test_identifier.clone(),
                create_statement,
                table_type: TableType::CoreBlockchain,
                commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags {
                    hyper_kzg: false,
                    dynamic_dory: true,
                }),
                source: sxt_core::tables::Source::Ethereum,
            }]
            .try_into()
            .unwrap(),
        )
        .unwrap();

        // Create an empty BatchId
        let test_batch = BatchId::try_from(b"".to_vec()).unwrap();
        let test_data = RowData::try_from(b"some arbitrary row data".to_vec()).unwrap();

        assert_err!(
            Indexing::submit_data(signer, test_identifier, test_batch, test_data,),
            crate::Error::<Test, Api>::InvalidBatch
        );
    })
}

#[test]
fn inserting_data_fails_when_batch_id_has_already_been_decided_on() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let (table_id, create_statement) = sample_table_definition();
        Tables::create_tables(
            RuntimeOrigin::root(),
            vec![UpdateTable {
                ident: table_id.clone(),
                create_statement,
                table_type: TableType::CoreBlockchain,
                commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags {
                    hyper_kzg: true,
                    dynamic_dory: true,
                }),
                source: sxt_core::tables::Source::Ethereum,
            }]
            .try_into()
            .unwrap(),
        )
        .unwrap();

        // Add permissions for the test accounts
        let permissions = PermissionList::try_from(vec![PermissionLevel::IndexingPallet(
            IndexingPalletPermission::SubmitDataForPublicQuorum,
        )])
        .unwrap();
        for id in 0..5 {
            let who = ensure_signed(RuntimeOrigin::signed(sp_runtime::AccountId32::new(
                [id; 32],
            )))
            .unwrap();
            pallet_permissions::Permissions::<Test>::insert(who, permissions.clone());
        }

        let test_batch_id = BatchId::try_from(b"test_batch".to_vec()).unwrap();
        let test_submission = TestSubmission {
            table: table_id.clone(),
            batch_id: test_batch_id.clone(),
            data: row_data(),
        };
        let data_hash = hash_row_data_with_block_number::<Test>(&test_submission.data, None);

        // Submit enough data to ensure the quorum is reached
        for i in 0..4 {
            assert_ok!(Indexing::submit_data(
                RuntimeOrigin::signed(sp_runtime::AccountId32::new([i; 32])),
                test_submission.table.clone(),
                test_submission.batch_id.clone(),
                test_submission.data.clone()
            ));
        }

        let internal_batch_id =
            build_inner_batch_id::<Test, Api>(&test_submission.batch_id, &table_id);

        // Verify that the data is finalized
        let maybe_data = Indexing::final_data(&internal_batch_id);
        assert!(maybe_data.is_some());
        let quorum = maybe_data.unwrap();
        assert_eq!(quorum.data_hash, data_hash);
        assert_eq!(quorum.table, test_submission.table);

        // Future submissions to this batch should receive the LateBatch Error
        let who = ensure_signed(RuntimeOrigin::signed(sp_runtime::AccountId32::new(
            [123; 32],
        )))
        .unwrap();
        pallet_permissions::Permissions::<Test>::insert(who.clone(), permissions.clone());
        assert_err!(
            Indexing::submit_data(
                RuntimeOrigin::signed(who),
                test_submission.table.clone(),
                test_submission.batch_id.clone(),
                test_submission.data.clone(),
            ),
            crate::Error::<Test, Api>::LateBatch
        );
    })
}

#[test]
fn submit_data_with_mothership_key_work() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (test_identifier, test_create) = sample_table_definition();
        Tables::create_tables(
            RuntimeOrigin::root(),
            vec![UpdateTable {
                ident: test_identifier.clone(),
                create_statement: test_create,
                table_type: TableType::Testing(InsertQuorumSize {
                    public: Some(1),
                    privileged: None,
                }),
                commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags {
                    hyper_kzg: true,
                    dynamic_dory: true,
                }),
                source: sxt_core::tables::Source::Ethereum,
            }]
            .try_into()
            .unwrap(),
        )
        .unwrap();

        let signer_key = sp_runtime::AccountId32::new([1; 32]);
        let signer = RuntimeOrigin::signed(signer_key.clone());
        let admin = sp_runtime::AccountId32::new([2; 32]);

        let admin_permission = PermissionLevel::EditSpecificPermission(Box::new(
            PermissionLevel::IndexingPallet(IndexingPalletPermission::SubmitDataForPublicQuorum),
        ));
        let permission_list = BoundedVec::try_from(vec![admin_permission]).unwrap();
        assert_ok!(pallet_permissions::Pallet::<Test>::set_permissions(
            RuntimeOrigin::root(),
            admin.clone(),
            permission_list,
        ));

        let permission =
            PermissionLevel::IndexingPallet(IndexingPalletPermission::SubmitDataForPublicQuorum);
        assert_ok!(pallet_permissions::Pallet::<Test>::add_proxy_permission(
            RuntimeOrigin::signed(admin),
            signer_key.clone(),
            permission,
        ));

        let test_batch = BatchId::try_from(b"test_batch".to_vec()).unwrap();
        let test_data = row_data();

        assert_ok!(Indexing::submit_data(
            signer.clone(),
            test_identifier.clone(),
            test_batch.clone(),
            test_data.clone(),
        ),);

        let hash = hash_row_data_with_block_number::<Test>(&test_data, None);

        let internal_batch_id = build_inner_batch_id::<Test, Api>(&test_batch, &test_identifier);
        // Verify that the submission was stored as expected
        // and the hash was generated from the submitted data
        assert_eq!(
            Indexing::submissions_v1((internal_batch_id, QuorumScope::Public, signer_key)),
            Some(hash)
        );
    })
}

#[test]
fn we_can_reach_privileged_quorum() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (table_id, create_statement) = sample_table_definition();
        Tables::create_tables(
            RuntimeOrigin::root(),
            vec![UpdateTable {
                ident: table_id.clone(),
                create_statement,
                table_type: TableType::Testing(InsertQuorumSize {
                    public: None,
                    privileged: Some(0),
                }),
                commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags {
                    hyper_kzg: true,
                    dynamic_dory: true,
                }),
                source: sxt_core::tables::Source::Ethereum,
            }]
            .try_into()
            .unwrap(),
        )
        .unwrap();

        let test_submission = TestSubmission {
            table: table_id.clone(),
            batch_id: BatchId::try_from(b"test_batch".to_vec()).unwrap(),
            data: row_data(),
        };
        let test_data_hash = hash_row_data_with_block_number::<Test>(&test_submission.data, None);

        // Add permissions for the test accounts
        let permissions = PermissionList::try_from(vec![PermissionLevel::IndexingPallet(
            IndexingPalletPermission::SubmitDataForPrivilegedQuorum(table_id.clone()),
        )])
        .unwrap();

        let origin = RuntimeOrigin::signed(sp_runtime::AccountId32::new([1; 32]));
        let who = ensure_signed(origin.clone()).unwrap();
        pallet_permissions::Permissions::<Test>::insert(who, permissions.clone());

        // Send the final required submission
        assert_ok!(submit_test_data(origin, test_submission.clone()));
        let internal_batch_id =
            build_inner_batch_id::<Test, Api>(&test_submission.batch_id, &table_id);

        let maybe_final_data = Indexing::final_data(&internal_batch_id);
        assert!(maybe_final_data.is_some());

        let fd = maybe_final_data.unwrap();
        assert_eq!(fd.data_hash, test_data_hash);
        assert_eq!(fd.table, test_submission.table);
        assert_eq!(fd.quorum_scope, QuorumScope::Privileged);

        // Verify that the old data was successfully removed for this batch
        let internal_batch_id =
            build_inner_batch_id::<Test, Api>(&test_submission.batch_id, &test_submission.table);
        let submitters_count =
            crate::SubmissionsV1::<Test, Api>::iter_prefix((internal_batch_id.clone(),)).count();
        assert_eq!(submitters_count, 0);
    })
}

#[test]
fn we_can_manage_quorum_state_for_both_scopes() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (table_id, create_statement) = sample_table_definition();
        Tables::create_tables(
            RuntimeOrigin::root(),
            vec![UpdateTable {
                ident: table_id.clone(),
                create_statement,
                table_type: TableType::Testing(InsertQuorumSize {
                    public: Some(2),
                    privileged: Some(1),
                }),
                commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags {
                    hyper_kzg: true,
                    dynamic_dory: true,
                }),
                source: sxt_core::tables::Source::Ethereum,
            }]
            .try_into()
            .unwrap(),
        )
        .unwrap();

        let test_submission = TestSubmission {
            table: table_id.clone(),
            batch_id: BatchId::try_from(b"test_batch".to_vec()).unwrap(),
            data: row_data(),
        };
        let test_data_hash = hash_row_data_with_block_number::<Test>(&row_data(), None);

        // Add permissions for the test accounts
        let public_permission =
            PermissionLevel::IndexingPallet(IndexingPalletPermission::SubmitDataForPublicQuorum);
        let privileged_permission = PermissionLevel::IndexingPallet(
            IndexingPalletPermission::SubmitDataForPrivilegedQuorum(table_id.clone()),
        );

        let public_submitter = RuntimeOrigin::signed(sp_runtime::AccountId32::new([1; 32]));
        let who = ensure_signed(public_submitter.clone()).unwrap();
        pallet_permissions::Permissions::<Test>::insert(
            who,
            PermissionList::try_from(vec![public_permission.clone()]).unwrap(),
        );

        let privileged_submitter = RuntimeOrigin::signed(sp_runtime::AccountId32::new([2; 32]));
        let who = ensure_signed(privileged_submitter.clone()).unwrap();
        pallet_permissions::Permissions::<Test>::insert(
            who,
            PermissionList::try_from(vec![privileged_permission.clone()]).unwrap(),
        );

        let both_submitter = RuntimeOrigin::signed(sp_runtime::AccountId32::new([3; 32]));
        let who = ensure_signed(both_submitter.clone()).unwrap();
        pallet_permissions::Permissions::<Test>::insert(
            who,
            PermissionList::try_from(vec![public_permission, privileged_permission]).unwrap(),
        );

        // public submission
        assert_ok!(submit_test_data(public_submitter, test_submission.clone()));

        let internal_batch_id =
            build_inner_batch_id::<Test, Api>(&test_submission.batch_id, &table_id);

        assert_eq!(
            crate::SubmissionsV1::<Test, Api>::iter_prefix((
                &internal_batch_id,
                QuorumScope::Public
            ))
            .count(),
            1
        );
        assert_eq!(
            crate::SubmissionsV1::<Test, Api>::iter_prefix((
                &internal_batch_id,
                QuorumScope::Privileged
            ))
            .count(),
            0
        );
        assert!(Indexing::final_data(&internal_batch_id).is_none());

        // both submission
        assert_ok!(submit_test_data(both_submitter, test_submission.clone()));

        assert_eq!(
            crate::SubmissionsV1::<Test, Api>::iter_prefix((
                &internal_batch_id,
                QuorumScope::Public
            ))
            .count(),
            1
        );
        assert_eq!(
            crate::SubmissionsV1::<Test, Api>::iter_prefix((
                &internal_batch_id,
                QuorumScope::Privileged
            ))
            .count(),
            1
        );
        assert!(Indexing::final_data(&internal_batch_id).is_none());

        // privileged submission
        assert_ok!(submit_test_data(
            privileged_submitter,
            test_submission.clone()
        ));
        let final_data = Indexing::final_data(&internal_batch_id).unwrap();

        assert_eq!(final_data.data_hash, test_data_hash);
        assert_eq!(final_data.table, test_submission.table);
        assert_eq!(final_data.quorum_scope, QuorumScope::Privileged);

        // Verify that the old data was successfully removed for this batch
        assert_eq!(
            crate::SubmissionsV1::<Test, Api>::iter_prefix((&internal_batch_id,)).count(),
            0
        );

        assert_eq!(
            System::read_events_for_pallet::<Event<Test, Api>>()
                .into_iter()
                .filter(|e| matches!(e, Event::QuorumReached { .. }))
                .count(),
            1
        );
    })
}

#[test]
fn reaching_quorum_for_both_scopes_simultaneously_produces_privileged_quorum_reached_event() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (table_id, create_statement) = sample_table_definition();
        Tables::create_tables(
            RuntimeOrigin::root(),
            vec![UpdateTable {
                ident: table_id.clone(),
                create_statement,
                table_type: TableType::Testing(InsertQuorumSize {
                    public: Some(0),
                    privileged: Some(0),
                }),
                commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags {
                    hyper_kzg: true,
                    dynamic_dory: true,
                }),
                source: sxt_core::tables::Source::Ethereum,
            }]
            .try_into()
            .unwrap(),
        )
        .unwrap();

        let test_submission = TestSubmission {
            table: table_id.clone(),
            batch_id: BatchId::try_from(b"test_batch".to_vec()).unwrap(),
            data: row_data(),
        };
        let test_data_hash = hash_row_data_with_block_number::<Test>(&test_submission.data, None);

        // Add permissions for the test accounts
        let public_permission =
            PermissionLevel::IndexingPallet(IndexingPalletPermission::SubmitDataForPublicQuorum);
        let privileged_permission = PermissionLevel::IndexingPallet(
            IndexingPalletPermission::SubmitDataForPrivilegedQuorum(table_id.clone()),
        );

        let both_submitter = RuntimeOrigin::signed(sp_runtime::AccountId32::new([3; 32]));
        let who = ensure_signed(both_submitter.clone()).unwrap();
        pallet_permissions::Permissions::<Test>::insert(
            who,
            PermissionList::try_from(vec![public_permission, privileged_permission]).unwrap(),
        );

        // both submission
        assert_ok!(submit_test_data(both_submitter, test_submission.clone()));

        let internal_batch_id =
            build_inner_batch_id::<Test, Api>(&test_submission.batch_id, &table_id);

        let final_data = Indexing::final_data(&internal_batch_id).unwrap();

        assert_eq!(final_data.data_hash, test_data_hash);
        assert_eq!(final_data.table, test_submission.table);
        // Privileged quorum is selected over public in this case
        assert_eq!(final_data.quorum_scope, QuorumScope::Privileged);

        // Verify that the old data was successfully removed for this batch
        assert_eq!(
            crate::SubmissionsV1::<Test, Api>::iter_prefix((&internal_batch_id,)).count(),
            0
        );

        assert_eq!(
            System::read_events_for_pallet::<Event<Test, Api>>()
                .into_iter()
                .filter(|e| matches!(e, Event::QuorumReached { .. }))
                .count(),
            1
        );
    })
}

#[test]
fn we_cannot_submit_for_table_disabled_quorum_scope() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (table_id, create_statement) = sample_table_definition();
        Tables::create_tables(
            RuntimeOrigin::root(),
            vec![UpdateTable {
                ident: table_id.clone(),
                create_statement,
                table_type: TableType::Testing(InsertQuorumSize {
                    public: None,
                    privileged: Some(0),
                }),
                commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags {
                    hyper_kzg: false,
                    dynamic_dory: true,
                }),
                source: sxt_core::tables::Source::Ethereum,
            }]
            .try_into()
            .unwrap(),
        )
        .unwrap();

        let test_submission = TestSubmission {
            table: table_id.clone(),
            batch_id: BatchId::try_from(b"test_batch".to_vec()).unwrap(),
            data: row_data(),
        };

        let public_permission =
            PermissionLevel::IndexingPallet(IndexingPalletPermission::SubmitDataForPublicQuorum);

        let public_submitter = RuntimeOrigin::signed(sp_runtime::AccountId32::new([1; 32]));
        let who = ensure_signed(public_submitter.clone()).unwrap();
        pallet_permissions::Permissions::<Test>::insert(
            who,
            PermissionList::try_from(vec![public_permission.clone()]).unwrap(),
        );

        // public submission
        assert_err!(
            submit_test_data(public_submitter, test_submission.clone()),
            crate::Error::<Test, Api>::UnauthorizedSubmitter
        );
        let internal_batch_id =
            build_inner_batch_id::<Test, Api>(&test_submission.batch_id, &test_submission.table);
        assert_eq!(
            crate::SubmissionsV1::<Test, Api>::iter_prefix((&internal_batch_id,)).count(),
            0
        );
        assert!(Indexing::final_data(&internal_batch_id).is_none());
    })
}

#[test]
fn we_cannot_submit_with_privilege_to_different_table() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (table_id, create_statement) = sample_table_definition();
        Tables::create_tables(
            RuntimeOrigin::root(),
            vec![UpdateTable {
                ident: table_id.clone(),
                create_statement,
                table_type: TableType::Testing(InsertQuorumSize {
                    public: None,
                    privileged: Some(0),
                }),
                commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags {
                    hyper_kzg: false,
                    dynamic_dory: true,
                }),
                source: sxt_core::tables::Source::Ethereum,
            }]
            .try_into()
            .unwrap(),
        )
        .unwrap();

        let test_submission = TestSubmission {
            table: table_id.clone(),
            batch_id: BatchId::try_from(b"test_batch".to_vec()).unwrap(),
            data: row_data(),
        };
        let test_data_hash =
            <<Test as frame_system::Config>::Hashing as Hasher>::hash(&test_submission.data);

        let incorrect_privileged_permission = PermissionLevel::IndexingPallet(
            IndexingPalletPermission::SubmitDataForPrivilegedQuorum(TableIdentifier::default()),
        );

        let privileged_submitter = RuntimeOrigin::signed(sp_runtime::AccountId32::new([1; 32]));
        let who = ensure_signed(privileged_submitter.clone()).unwrap();
        pallet_permissions::Permissions::<Test>::insert(
            who,
            PermissionList::try_from(vec![incorrect_privileged_permission.clone()]).unwrap(),
        );

        // privileged submission
        assert_err!(
            submit_test_data(privileged_submitter, test_submission.clone()),
            crate::Error::<Test, Api>::UnauthorizedSubmitter
        );
        let internal_batch_id =
            build_inner_batch_id::<Test, Api>(&test_submission.batch_id, &test_submission.table);
        assert_eq!(
            crate::SubmissionsV1::<Test, Api>::iter_prefix((&internal_batch_id,)).count(),
            0
        );
        assert!(Indexing::final_data(&test_submission.batch_id).is_none());
    })
}

#[test]
fn blockchain_data_submission_stores_block_number() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (table_id, create_stmt) = sample_table_definition();

        Tables::create_tables(
            RuntimeOrigin::root(),
            vec![UpdateTable {
                ident: table_id.clone(),
                create_statement: create_stmt,
                table_type: TableType::Testing(InsertQuorumSize {
                    public: Some(0),
                    privileged: None,
                }),
                commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags {
                    hyper_kzg: true,
                    dynamic_dory: true,
                }),
                source: sxt_core::tables::Source::Ethereum,
            }]
            .try_into()
            .unwrap(),
        )
        .unwrap();

        let signer = RuntimeOrigin::signed(sp_runtime::AccountId32::new([1; 32]));
        let who = ensure_signed(signer.clone()).unwrap();

        pallet_permissions::Permissions::<Test>::insert(
            who,
            PermissionList::try_from(vec![PermissionLevel::IndexingPallet(
                IndexingPalletPermission::SubmitDataForPublicQuorum,
            )])
            .unwrap(),
        );

        let batch = BatchId::try_from(b"blockchain_batch".to_vec()).unwrap();
        let data = row_data();
        let block_number = 12345;

        assert_ok!(Indexing::submit_blockchain_data(
            signer,
            table_id.clone(),
            batch.clone(),
            data.clone(),
            block_number
        ));

        let stored = Indexing::block_numbers(&table_id);
        assert_eq!(stored, Some(block_number));
    });
}

#[test]
fn empty_blockchain_data_emits_empty_event_with_block_number() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (table_id, create_stmt) = sample_table_definition();

        Tables::create_tables(
            RuntimeOrigin::root(),
            vec![UpdateTable {
                ident: table_id.clone(),
                create_statement: create_stmt,
                table_type: TableType::Testing(InsertQuorumSize {
                    public: Some(0),
                    privileged: None,
                }),
                commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags {
                    hyper_kzg: true,
                    dynamic_dory: true,
                }),
                source: sxt_core::tables::Source::Ethereum,
            }]
            .try_into()
            .unwrap(),
        )
        .unwrap();

        let signer = RuntimeOrigin::signed(sp_runtime::AccountId32::new([1; 32]));
        let who = ensure_signed(signer.clone()).unwrap();

        pallet_permissions::Permissions::<Test>::insert(
            who,
            PermissionList::try_from(vec![PermissionLevel::IndexingPallet(
                IndexingPalletPermission::SubmitDataForPublicQuorum,
            )])
            .unwrap(),
        );

        let batch = BatchId::try_from(b"empty_block_batch".to_vec()).unwrap();
        let empty_data = empty_row_data();
        let block_number = 54321;

        assert_ok!(Indexing::submit_blockchain_data(
            signer,
            table_id.clone(),
            batch.clone(),
            empty_data,
            block_number
        ));

        let events = System::read_events_for_pallet::<Event<Test, Api>>();
        assert!(events.iter().any(
            |event| matches!(event, Event::QuorumEmptyBlock { table, block_number: bn, .. }
                if table == &table_id && *bn == block_number)
        ));
    });
}

#[test]
fn fallback_to_oc_table_block_number_when_none_provided() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (table_id, create_stmt) = sample_table_definition_with_block_number();

        Tables::create_tables(
            RuntimeOrigin::root(),
            vec![UpdateTable {
                ident: table_id.clone(),
                create_statement: create_stmt,
                table_type: TableType::Testing(InsertQuorumSize {
                    public: Some(0),
                    privileged: None,
                }),
                commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags {
                    hyper_kzg: true,
                    dynamic_dory: true,
                }),
                source: sxt_core::tables::Source::Ethereum,
            }]
            .try_into()
            .unwrap(),
        )
        .unwrap();

        let signer = RuntimeOrigin::signed(sp_runtime::AccountId32::new([1; 32]));
        let who = ensure_signed(signer.clone()).unwrap();
        pallet_permissions::Permissions::<Test>::insert(
            who,
            PermissionList::try_from(vec![PermissionLevel::IndexingPallet(
                IndexingPalletPermission::SubmitDataForPublicQuorum,
            )])
            .unwrap(),
        );

        let batch = BatchId::try_from(b"fallback_batch".to_vec()).unwrap();
        let data = row_data_w_block_number();

        // Submit via `submit_data` without providing block_number
        assert_ok!(Indexing::submit_data(
            RuntimeOrigin::signed(sp_runtime::AccountId32::new([1; 32])),
            table_id.clone(),
            batch.clone(),
            data.clone()
        ));

        // Ensure a block number was stored (should be derived from `max_block_number`)
        let stored = Indexing::block_numbers(&table_id);
        assert!(stored.is_some());
    });
}

#[test]
fn no_block_number_stored_when_implicit_and_empty_data() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (table_id, create_stmt) = sample_table_definition();

        Tables::create_tables(
            RuntimeOrigin::root(),
            vec![UpdateTable {
                ident: table_id.clone(),
                create_statement: create_stmt,
                table_type: TableType::Testing(InsertQuorumSize {
                    public: Some(0),
                    privileged: None,
                }),
                commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags {
                    hyper_kzg: true,
                    dynamic_dory: true,
                }),
                source: sxt_core::tables::Source::Ethereum,
            }]
            .try_into()
            .unwrap(),
        )
        .unwrap();

        let signer = RuntimeOrigin::signed(sp_runtime::AccountId32::new([1; 32]));
        let who = ensure_signed(signer.clone()).unwrap();

        pallet_permissions::Permissions::<Test>::insert(
            who,
            PermissionList::try_from(vec![PermissionLevel::IndexingPallet(
                IndexingPalletPermission::SubmitDataForPublicQuorum,
            )])
            .unwrap(),
        );

        let batch = BatchId::try_from(b"empty_implicit".to_vec()).unwrap();
        let empty_data = row_data();

        // Uses `submit_data` (no explicit block_number)
        assert_ok!(Indexing::submit_data(
            signer,
            table_id.clone(),
            batch,
            empty_data,
        ));

        let stored = Indexing::block_numbers(&table_id);
        assert_eq!(stored, None);
    });
}

#[test]
fn we_can_reach_quorum_before_and_after_changing_quorum_size() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (table_id, create_statement) = sample_table_definition();

        let test_quorum = InsertQuorumSize {
            public: None,
            privileged: Some(0),
        };
        Tables::create_tables(
            RuntimeOrigin::root(),
            vec![UpdateTable {
                ident: table_id.clone(),
                create_statement,
                table_type: TableType::Testing(test_quorum),
                commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags {
                    hyper_kzg: true,
                    dynamic_dory: true,
                }),
                source: sxt_core::tables::Source::Ethereum,
            }]
            .try_into()
            .unwrap(),
        )
        .unwrap();

        let test_submission = TestSubmission {
            table: table_id.clone(),
            batch_id: BatchId::try_from(b"test_batch".to_vec()).unwrap(),
            data: row_data_with_count(1),
        };
        let test_data_hash = hash_row_data_with_block_number::<Test>(&test_submission.data, None);

        // Add permissions for the test accounts
        let permissions = PermissionList::try_from(vec![PermissionLevel::IndexingPallet(
            IndexingPalletPermission::SubmitDataForPrivilegedQuorum(table_id.clone()),
        )])
        .unwrap();

        let origin = RuntimeOrigin::signed(sp_runtime::AccountId32::new([1; 32]));
        let who = ensure_signed(origin.clone()).unwrap();
        pallet_permissions::Permissions::<Test>::insert(who, permissions.clone());

        // Send the final required submission
        assert_ok!(submit_test_data(origin, test_submission.clone()));

        let internal_batch_id =
            build_inner_batch_id::<Test, Api>(&test_submission.batch_id, &table_id);

        let maybe_final_data = Indexing::final_data(&internal_batch_id);
        assert!(maybe_final_data.is_some());

        let fd = maybe_final_data.unwrap();
        assert_eq!(fd.data_hash, test_data_hash);
        assert_eq!(fd.table, test_submission.table);
        assert_eq!(fd.quorum_scope, QuorumScope::Privileged);

        let internal_batch_id =
            build_inner_batch_id::<Test, Api>(&test_submission.batch_id, &table_id);

        // Verify that the old data was successfully removed for this batch
        let submitters_count =
            crate::SubmissionsV1::<Test, Api>::iter_prefix((internal_batch_id.clone(),)).count();
        assert_eq!(submitters_count, 0);

        // Now update the quorum to make this table public
        let new_quorum = InsertQuorumSize {
            privileged: None,
            public: Some(0),
        };

        assert_ok!(Tables::update_table_quorum(
            RuntimeOrigin::root(),
            table_id.clone(),
            new_quorum
        ));

        // Ensure the event was emitted as expected
        System::assert_has_event(RuntimeEvent::Tables(
            pallet_tables::Event::<Test>::QuorumUpdated {
                table: table_id.clone(),
                old_quorum: Some(test_quorum),
                new_quorum,
            },
        ));

        // Now submit with an account that has "public" data submission permissions
        let test_submission_2 = TestSubmission {
            table: table_id.clone(),
            batch_id: BatchId::try_from(b"test_batch2".to_vec()).unwrap(),
            data: row_data_with_count(1),
        };
        let test_data_hash = hash_row_data_with_block_number::<Test>(&test_submission.data, None);

        // Add permissions for the test accounts
        let permissions = PermissionList::try_from(vec![PermissionLevel::IndexingPallet(
            IndexingPalletPermission::SubmitDataForPublicQuorum,
        )])
        .unwrap();

        let origin = RuntimeOrigin::signed(sp_runtime::AccountId32::new([2; 32]));
        let who = ensure_signed(origin.clone()).unwrap();
        pallet_permissions::Permissions::<Test>::insert(who, permissions.clone());

        // Send the data
        assert_ok!(submit_test_data(origin, test_submission_2.clone()));

        let internal_batch_id_2 =
            build_inner_batch_id::<Test, Api>(&test_submission_2.batch_id, &table_id);

        let maybe_final_data = Indexing::final_data(&internal_batch_id_2);

        assert!(maybe_final_data.is_some());

        let fd = maybe_final_data.unwrap();
        assert_eq!(fd.data_hash, test_data_hash);
        assert_eq!(fd.table, test_submission.table);
        assert_eq!(fd.quorum_scope, QuorumScope::Public);

        // Verify that the old data was successfully removed for this batch
        let submitters_count =
            crate::SubmissionsV1::<Test, Api>::iter_prefix((internal_batch_id_2.clone(),)).count();
        assert_eq!(submitters_count, 0);
    });
}

#[test]
fn we_can_submit_to_permissionless_table_with_no_permissions() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (table_id, create_statement) = sample_table_definition();
        Tables::create_tables(
            RuntimeOrigin::root(),
            vec![UpdateTable {
                ident: table_id.clone(),
                create_statement,
                table_type: TableType::PublicPermissionless,
                commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags {
                    hyper_kzg: false,
                    dynamic_dory: true,
                }),
                source: sxt_core::tables::Source::Ethereum,
            }]
            .try_into()
            .unwrap(),
        )
        .unwrap();

        let test_submission = TestSubmission {
            table: table_id.clone(),
            batch_id: BatchId::try_from(b"test_batch".to_vec()).unwrap(),
            data: row_data(),
        };
        let test_data_hash =
            <<Test as frame_system::Config>::Hashing as Hasher>::hash(&test_submission.data);

        let public_submitter = RuntimeOrigin::signed(sp_runtime::AccountId32::new([1; 32]));
        let who = ensure_signed(public_submitter.clone()).unwrap();

        // permissionless submission
        assert_ok!(submit_test_data(public_submitter, test_submission.clone()));

        let internal_batch_id =
            build_inner_batch_id::<Test, Api>(&test_submission.batch_id, &table_id);
        assert!(Indexing::final_data(&internal_batch_id).is_some());
    })
}

#[test]
fn submitters_can_overwrite_their_submission() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (table_id, create_statement) = sample_table_definition();
        Tables::create_tables(
            RuntimeOrigin::root(),
            vec![UpdateTable {
                ident: table_id.clone(),
                create_statement,
                table_type: TableType::CoreBlockchain,
                commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::all()),
                source: sxt_core::tables::Source::Ethereum,
            }]
            .try_into()
            .unwrap(),
        )
        .unwrap();

        let permissions = PermissionList::try_from(vec![PermissionLevel::IndexingPallet(
            IndexingPalletPermission::SubmitDataForPublicQuorum,
        )])
        .unwrap();
        let signer = RuntimeOrigin::signed(sp_runtime::AccountId32::new([1; 32]));
        let who = ensure_signed(signer.clone()).unwrap();
        pallet_permissions::Permissions::<Test>::insert(&who, permissions);

        let batch_id = BatchId::try_from(b"test_batch".to_vec()).unwrap();
        let data = row_data();
        let data_hash = hash_row_data_with_block_number::<Test>(&data, None);

        Indexing::submit_data(
            signer.clone(),
            table_id.clone(),
            batch_id.clone(),
            data.clone(),
        )
        .unwrap();
        let internal_batch_id = build_inner_batch_id::<Test, Api>(&batch_id, &table_id);
        assert_eq!(
            crate::SubmissionsV1::<Test, Api>::get((&internal_batch_id, QuorumScope::Public, &who))
                .unwrap(),
            data_hash
        );

        let different_data = diff_row_data();
        let different_data_hash = hash_row_data_with_block_number::<Test>(&different_data, None);
        Indexing::submit_data(signer, table_id, batch_id.clone(), different_data.clone()).unwrap();
        assert_eq!(
            crate::SubmissionsV1::<Test, Api>::get((&internal_batch_id, QuorumScope::Public, &who))
                .unwrap(),
            different_data_hash
        );
    });
}

#[test]
fn submitters_cannot_exceed_maximum() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (table_id, create_statement) = sample_table_definition();
        Tables::create_tables(
            RuntimeOrigin::root(),
            vec![UpdateTable {
                ident: table_id.clone(),
                create_statement,
                table_type: TableType::CoreBlockchain,
                commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::all()),
                source: sxt_core::tables::Source::Ethereum,
            }]
            .try_into()
            .unwrap(),
        )
        .unwrap();

        let batch_id = BatchId::try_from(b"test_batch".to_vec()).unwrap();

        let internal_batch_id = build_inner_batch_id::<Test, Api>(&batch_id, &table_id);
        // artificially fill submissions for batch
        (1..=MAX_SUBMITTERS).for_each(|submitter_num| {
            let signer =
                RuntimeOrigin::signed(sp_runtime::AccountId32::new([submitter_num as u8; 32]));
            let who = ensure_signed(signer.clone()).unwrap();
            let artificial_data_hash = <<Test as frame_system::Config>::Hashing as Hasher>::hash(
                &submitter_num.to_le_bytes(),
            );

            crate::SubmissionsV1::<Test, Api>::insert(
                (internal_batch_id.clone(), QuorumScope::Public, who),
                artificial_data_hash,
            );
        });

        // we cannot insert one more
        let permissions = PermissionList::try_from(vec![PermissionLevel::IndexingPallet(
            IndexingPalletPermission::SubmitDataForPublicQuorum,
        )])
        .unwrap();

        let signer = RuntimeOrigin::signed(sp_runtime::AccountId32::new(
            [(MAX_SUBMITTERS + 1) as u8; 32],
        ));
        let who = ensure_signed(signer.clone()).unwrap();
        pallet_permissions::Permissions::<Test>::insert(who, permissions.clone());
        let data = row_data();

        assert_noop!(
            Indexing::submit_data(signer.clone(), table_id.clone(), batch_id.clone(), data,),
            crate::Error::<Test, Api>::MaxSubmittersReached
        );

        // submitters can still re-submit new hashes
        let signer = RuntimeOrigin::signed(sp_runtime::AccountId32::new([1; 32]));
        let who = ensure_signed(signer.clone()).unwrap();
        pallet_permissions::Permissions::<Test>::insert(&who, permissions);
        let data = row_data();
        let data_hash = hash_row_data_with_block_number::<Test>(&data, None);

        Indexing::submit_data(
            signer.clone(),
            table_id.clone(),
            batch_id.clone(),
            data.clone(),
        )
        .unwrap();
        assert_eq!(
            crate::SubmissionsV1::<Test, Api>::get((&internal_batch_id, QuorumScope::Public, &who))
                .unwrap(),
            data_hash
        );
    });
}

/// Returns an `AccountId32` seeded by a `usize`.
fn account_from_num(index: usize) -> sp_runtime::AccountId32 {
    let bytes = index
        .to_le_bytes()
        .into_iter()
        .chain(std::iter::repeat(0))
        .take(32)
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    sp_runtime::AccountId32::new(bytes)
}

/// Returns a `QuorumScope` seeded by a `usize`
fn quorum_scope_public_if_even(index: usize) -> QuorumScope {
    if index % 2 == 0 {
        QuorumScope::Public
    } else {
        QuorumScope::Privileged
    }
}

/// Populates the `Submissions` (v0) storage with the given test submissions.
///
/// The account and quorum scope used for the storage is determined by seeding the `account_fn` and
/// `quorum_scope_fn`.
fn populate_submissions_v0<T, I>(
    mut account_fn: impl FnMut(usize) -> T::AccountId,
    mut quorum_scope_fn: impl FnMut(usize) -> QuorumScope,
    submissions: impl IntoIterator<Item = TestSubmission>,
) where
    T: Config<I>,
    I: NativeApi,
{
    submissions
        .into_iter()
        .enumerate()
        .for_each(|(i, test_submission)| {
            let account = account_fn(i);
            let internal_batch =
                build_inner_batch_id::<T, I>(&test_submission.batch_id, &test_submission.table);

            let data_hash = hash_row_data_with_block_number::<T>(&test_submission.data, None);

            let quorum_scope = quorum_scope_fn(i);

            crate::Submissions::<T, I>::insert(
                internal_batch,
                data_hash,
                SubmittersByScope::<T::AccountId>::default()
                    .with_submitter(account, &quorum_scope)
                    .unwrap(),
            )
        })
}

/// Submits the given test submissions.
///
/// The account and their quorum scope is determined by seeding the `account_fn` and
/// `quorum_scope_fn`.
fn submit_submissions_v1<T, I>(
    mut account_fn: impl FnMut(usize) -> T::AccountId,
    mut quorum_scope_fn: impl FnMut(usize) -> QuorumScope,
    submissions: impl IntoIterator<Item = TestSubmission>,
) where
    T: Config<I>,
    I: NativeApi,
{
    submissions
        .into_iter()
        .enumerate()
        .for_each(|(i, test_submission)| {
            let account = account_fn(i);
            let quorum_scope = quorum_scope_fn(i);

            let origin = RawOrigin::<T::AccountId>::Signed(account.clone()).into();

            match quorum_scope {
                QuorumScope::Public => {
                    let permission = PermissionLevel::IndexingPallet(
                        IndexingPalletPermission::SubmitDataForPublicQuorum,
                    );
                    if !pallet_permissions::Pallet::<T>::has_permissions(&account, &permission) {
                        pallet_permissions::Pallet::<T>::add_proxy_permission(
                            RawOrigin::Root.into(),
                            account,
                            permission,
                        )
                        .unwrap();
                    }
                }
                QuorumScope::Privileged => {
                    let permission = PermissionLevel::IndexingPallet(
                        IndexingPalletPermission::SubmitDataForPrivilegedQuorum(
                            test_submission.table.clone(),
                        ),
                    );
                    if !pallet_permissions::Pallet::<T>::has_permissions(&account, &permission) {
                        pallet_permissions::Pallet::<T>::add_proxy_permission(
                            RawOrigin::Root.into(),
                            account,
                            permission,
                        )
                        .unwrap();
                    }
                }
            }

            crate::Pallet::<T, I>::submit_data(
                origin,
                test_submission.table,
                test_submission.batch_id,
                test_submission.data,
            )
            .unwrap();
        })
}

/// Creates the [`sample_table_definition`] table and submits the given `TestSubmission`s for it.
///
/// Used primarily for pruning tests, hence both versions of the submission storage can be
/// parameterized.
fn setup_sample_table_with_submissions(
    submissions_v0: impl IntoIterator<Item = TestSubmission>,
    submissions_v1: impl IntoIterator<Item = TestSubmission>,
) {
    let (table_id, create_statement) = sample_table_definition();
    Tables::create_tables(
        RuntimeOrigin::root(),
        vec![UpdateTable {
            ident: table_id.clone(),
            create_statement,
            table_type: TableType::Testing(InsertQuorumSize {
                public: Some(2),
                privileged: Some(2),
            }),
            commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::all()),
            source: sxt_core::tables::Source::Ethereum,
        }]
        .try_into()
        .unwrap(),
    )
    .unwrap();

    populate_submissions_v0::<Test, Api>(
        account_from_num,
        quorum_scope_public_if_even,
        submissions_v0,
    );
    submit_submissions_v1::<Test, Api>(
        account_from_num,
        quorum_scope_public_if_even,
        submissions_v1,
    );
}

/// Returns the number of `BatchId`s in `SubmissionsV1` storage.
fn count_submissions_v1_batch_ids<T, I>() -> usize
where
    T: Config<I>,
    I: NativeApi,
{
    crate::SubmissionsV1::<T, I>::iter_keys()
        .map(|(batch_id, ..)| batch_id)
        .collect::<HashSet<_>>()
        .len()
}

proptest! {
    #[test]
    fn submissions_v0_are_always_pruned(
        submissions_v0 in {
            let max_batches_finding_quorum =
            <<Test as Config<Api>>::MaxBatchesFindingQuorum as Get<u32>>::get() as usize;
            submissions_for_sample_table(0..max_batches_finding_quorum, 0..4usize, batch_id_strategy())
        },
        // won't trigger v1 submission pruning
        submissions_v1 in {
            let max_batches_finding_quorum =
            <<Test as Config<Api>>::MaxBatchesFindingQuorum as Get<u32>>::get() as usize;
            submissions_for_sample_table_by_batch_id(0..max_batches_finding_quorum, 1..=32usize, 0..4usize, batch_id_strategy())
        },
    ) {
        new_test_ext().execute_with(|| {
            System::set_block_number(1);
            setup_sample_table_with_submissions(submissions_v0.clone(), submissions_v1.clone().into_values().flatten());

            assert_eq!(crate::Submissions::<Test, Api>::iter().count(), submissions_v0.len());
            assert_eq!(count_submissions_v1_batch_ids::<Test, Api>(), submissions_v1.len());
            assert_eq!(crate::BatchQueueBottom::<Test, Api>::get(), 0);
            assert_eq!(crate::BatchQueue::<Test, Api>::count(), submissions_v1.len() as u32);

            crate::Pallet::<Test, Api>::on_initialize(2);

            let max_batches_pruned = <<Test as Config<Api>>::MaxBatchesPruned as Get<u32>>::get();
            assert_eq!(crate::Submissions::<Test, Api>::iter().count(), submissions_v0.len().saturating_sub(max_batches_pruned as usize));
            assert_eq!(count_submissions_v1_batch_ids::<Test, Api>(), submissions_v1.len());
            assert_eq!(crate::BatchQueueBottom::<Test, Api>::get(), 0);
            assert_eq!(crate::BatchQueue::<Test, Api>::count(), submissions_v1.len() as u32);

            let expected_num_pruned_total = (submissions_v0.len() as u32).min(max_batches_pruned);
            if expected_num_pruned_total > 0 {
                let events = System::read_events_for_pallet::<Event<Test, Api>>();
                assert!(events.iter().any(
                    |event| matches!(event, Event::BatchQueuePruned { num_pruned }
                        if *num_pruned == expected_num_pruned_total)
                ));
            }
        });
    }

    #[test]
    fn submissions_v1_are_pruned_after_submissions_v0(
        // Usually pruning of v0 won't satisfy the max batches pruned
        submissions_v0 in {
            let max_batches_pruned =
            <<Test as Config<Api>>::MaxBatchesPruned as Get<u32>>::get() as usize;
            submissions_for_sample_table(0..max_batches_pruned, 0..4usize, batch_id_strategy())
        },
        // Pruning of v1 will begin because max_batches_finding_quorum is exceeded
        submissions_v1 in {
            let max_batches_finding_quorum =
            <<Test as Config<Api>>::MaxBatchesFindingQuorum as Get<u32>>::get() as usize;
            submissions_for_sample_table_by_batch_id(max_batches_finding_quorum..max_batches_finding_quorum*2, 1..=32usize, 0..4usize, batch_id_strategy())
        },
    ) {
        new_test_ext().execute_with(|| {
            System::set_block_number(1);
            setup_sample_table_with_submissions(submissions_v0.clone(), submissions_v1.clone().into_values().flatten());

            assert_eq!(crate::Submissions::<Test, Api>::iter().count(), submissions_v0.len());
            assert_eq!(count_submissions_v1_batch_ids::<Test, Api>(), submissions_v1.len());
            assert_eq!(crate::BatchQueueBottom::<Test, Api>::get(), 0);
            assert_eq!(crate::BatchQueue::<Test, Api>::count(), submissions_v1.len() as u32);

            crate::Pallet::<Test, Api>::on_initialize(2);

            assert_eq!(crate::Submissions::<Test, Api>::iter().count(), 0);

            let max_batches_finding_quorum =
                <<Test as Config<Api>>::MaxBatchesFindingQuorum as Get<u32>>::get();
            let max_batches_pruned = <<Test as Config<Api>>::MaxBatchesPruned as Get<u32>>::get();

            let expected_prune_limit = max_batches_pruned - submissions_v0.len() as u32;
            let excessive_batches = submissions_v1.len() as u32 - max_batches_finding_quorum;

            let expected_num_pruned = excessive_batches.min(expected_prune_limit);

            assert_eq!(count_submissions_v1_batch_ids::<Test, Api>(), submissions_v1.len() - expected_num_pruned as usize);
            assert_eq!(crate::BatchQueueBottom::<Test, Api>::get(), expected_num_pruned);
            assert_eq!(crate::BatchQueue::<Test, Api>::count(), submissions_v1.len() as u32 - expected_num_pruned);

            let expected_num_pruned_total = expected_num_pruned + submissions_v0.len() as u32;

            if expected_num_pruned_total > 0 {
                let events = System::read_events_for_pallet::<Event<Test, Api>>();
                assert!(events.iter().any(
                    |event| matches!(event, Event::BatchQueuePruned { num_pruned }
                        if *num_pruned == expected_num_pruned_total)
                ));
            }
        });
    }
}
