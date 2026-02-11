use alloc::boxed::Box;
use alloc::vec;
use std::convert::Into;
use std::io::Cursor;
use std::sync::Arc;

use arrow::array::{ArrayRef, Int32Array, Int64Array, RecordBatch};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::ipc::writer::StreamWriter;
use arrow_ipc_no_std::single_batch_stream_bytes;
use codec::{Decode, Encode, MaxEncodedLen};
use native_api::Api;
use pallet_tables::{CommitmentCreationCmd, UpdateTable};
use polkadot_sdk::frame_support::__private::RuntimeDebug;
use polkadot_sdk::frame_support::dispatch::DispatchResult;
use polkadot_sdk::frame_support::pallet_prelude::TypeInfo;
use polkadot_sdk::frame_support::{assert_err, assert_ok};
use polkadot_sdk::frame_system::ensure_signed;
use polkadot_sdk::sp_core::Hasher;
use polkadot_sdk::sp_runtime::BoundedVec;
use polkadot_sdk::{frame_system, sp_runtime};
use proof_of_sql_commitment_map::CommitmentSchemeFlags;
use proptest::prelude::*;
use sxt_core::permissions::{IndexingPalletPermission, PermissionLevel, PermissionList};
use sxt_core::proptest::{canonical_record_batch, DataCorruption};
use sxt_core::tables::{
    CreateStatement,
    InsertQuorumSize,
    QuorumScope,
    TableIdentifier,
    TableName,
    TableNamespace,
    TableType,
};

use crate::mock::*;
use crate::{build_inner_batch_id, BatchId, Event, RowData};

/// Used as a convenience wrapper for data we need to submit
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
struct TestSubmission {
    table: TableIdentifier,
    batch_id: BatchId,
    data: RowData,
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

    record_batch_to_row_data(batch, schema)
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

    record_batch_to_row_data(batch, schema)
}

fn record_batch_to_row_data(batch: RecordBatch, schema: Arc<Schema>) -> RowData {
    let buffer: Vec<u8> = Vec::new();
    let mut cursor = Cursor::new(buffer);

    let mut writer = StreamWriter::try_new(&mut cursor, &schema).unwrap();

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

fn empty_row_data() -> RowData {
    let schema = Arc::new(Schema::new(vec![Field::new(
        "int_column",
        DataType::Int32,
        false,
    )]));

    let empty_batch = RecordBatch::new_empty(schema.clone());

    record_batch_to_row_data(empty_batch, schema)
}

fn row_data_w_block_number() -> RowData {
    let schema = Arc::new(Schema::new(vec![
        Field::new("int_column", DataType::Int32, false),
        Field::new("block_number", DataType::Int64, false),
    ]));

    let int_data = Arc::new(Int32Array::from(vec![1, 2, 3, 4])) as ArrayRef;
    let block_data = Arc::new(Int64Array::from(vec![100, 101, 102, 12345])) as ArrayRef;

    let batch = RecordBatch::try_new(schema.clone(), vec![int_data, block_data]).unwrap();

    record_batch_to_row_data(batch, schema)
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
        pallet_permissions::Permissions::<Test>::insert(who, permissions.clone());

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
            Indexing::submissions(internal_batch_id, hash).len_of_scope(&QuorumScope::Public),
            1
        );
    })
}

#[test]
fn submission_fails_when_data_is_already_submitted() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (table_id, test_create) = sample_table_definition();
        Tables::create_tables(
            RuntimeOrigin::root(),
            vec![UpdateTable {
                ident: table_id.clone(),
                create_statement: test_create,
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
        let signer = RuntimeOrigin::signed(sp_runtime::AccountId32::new([1; 32]));
        let who = ensure_signed(signer.clone()).unwrap();
        let permissions = PermissionList::try_from(vec![PermissionLevel::IndexingPallet(
            IndexingPalletPermission::SubmitDataForPublicQuorum,
        )])
        .unwrap();
        pallet_permissions::Permissions::<Test>::insert(who, permissions.clone());

        let test_batch = BatchId::try_from(b"test_batch".to_vec()).unwrap();
        let test_data = row_data();

        assert_ok!(Indexing::submit_data(
            signer.clone(),
            table_id.clone(),
            test_batch.clone(),
            test_data.clone(),
        ),);

        let mut hash_input = test_data.encode();
        hash_input.extend(None::<u64>.encode());
        let hash = <<Test as frame_system::Config>::Hashing as Hasher>::hash(&hash_input);

        let internal_batch_id = build_inner_batch_id::<Test, Api>(&test_batch, &table_id);

        // Verify that the submission was stored as expected
        // and the hash was generated from the submitted data
        assert_eq!(
            Indexing::submissions(internal_batch_id.clone(), hash)
                .len_of_scope(&QuorumScope::Public),
            1
        );

        // Verify that submitting the same thing again returns the expected error
        assert_err!(
            Indexing::submit_data(
                signer.clone(),
                table_id.clone(),
                test_batch.clone(),
                test_data.clone(),
            ),
            crate::Error::<Test, Api>::AlreadySubmitted
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
        let signer = RuntimeOrigin::signed(sp_runtime::AccountId32::new([1; 32]));
        assert_err!(
            Indexing::submit_data(
                signer.clone(),
                test_identifier.clone(),
                test_batch.clone(),
                test_data.clone(),
            ),
            crate::Error::<Test, Api>::UnauthorizedSubmitter,
        );

        let hash = <<Test as frame_system::Config>::Hashing as Hasher>::hash(&test_data);

        // Verify that the submission was not stored
        assert_eq!(
            Indexing::submissions(test_batch.clone(), hash).len_of_scope(&QuorumScope::Public),
            0
        );
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
        let submitters = Indexing::submissions(&internal_batch_id, test_data_hash);
        assert!(submitters.scope_is_empty(&QuorumScope::Public));
        assert!(submitters.scope_is_empty(&QuorumScope::Privileged));
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
        for _i in 1..4 {
            assert!(Indexing::submissions(&internal_batch_id, data_hash)
                .scope_is_empty(&QuorumScope::Public))
        }
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
fn inserting_data_fails_when_data_is_not_a_record_batch() {
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
        let test_data = RowData::try_from(vec![0, 1, 2, 3]).unwrap();

        assert_err!(
            Indexing::submit_data(signer, test_identifier, test_batch, test_data,),
            crate::Error::<Test, Api>::ArrowParseSchemaMessage
        );
    })
}

proptest! {
    // generating the (up to) 64x64 record batches for this test is a bit slow
    // Default value for this is 256, so this halves the test time and test coverage.
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn inserting_corrupted_record_batch_does_not_panic(
        record_batch in canonical_record_batch(),
        corruptions in proptest::collection::vec(any::<DataCorruption>(), 0..1024)
    ) {
        new_test_ext().execute_with(|| {
            let record_batch_bytes = single_batch_stream_bytes(&record_batch).unwrap();
            let corrupted_bytes = corruptions
                .iter()
                .fold(record_batch_bytes, |data, corruption| {
                    DataCorruption::corrupt(corruption, data)
                })
                .try_into()
                .unwrap();

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

            let _no_panic = Indexing::submit_data(signer, test_identifier, test_batch, corrupted_bytes);
        })
    }
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

        let (table_id, _create_statement) = sample_table_definition();
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
            signer_key,
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
            Indexing::submissions(internal_batch_id, hash).len_of_scope(&QuorumScope::Public),
            1
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
        let submitters = Indexing::submissions(&internal_batch_id, test_data_hash);
        assert!(submitters.scope_is_empty(&QuorumScope::Public));
        assert!(submitters.scope_is_empty(&QuorumScope::Privileged));
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

        let submissions = Indexing::submissions(&internal_batch_id, test_data_hash);
        assert_eq!(submissions.len_of_scope(&QuorumScope::Public), 1);
        assert!(submissions.scope_is_empty(&QuorumScope::Privileged));
        assert!(Indexing::final_data(&internal_batch_id).is_none());

        // both submission
        assert_ok!(submit_test_data(both_submitter, test_submission.clone()));

        let submissions = Indexing::submissions(&internal_batch_id, test_data_hash);
        assert_eq!(submissions.len_of_scope(&QuorumScope::Public), 1);
        assert_eq!(submissions.len_of_scope(&QuorumScope::Privileged), 1);
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
        let submitters = Indexing::submissions(&internal_batch_id, test_data_hash);
        assert!(submitters.scope_is_empty(&QuorumScope::Public));
        assert!(submitters.scope_is_empty(&QuorumScope::Privileged));

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
        let submitters = Indexing::submissions(test_submission.batch_id.clone(), test_data_hash);
        assert!(submitters.scope_is_empty(&QuorumScope::Public));
        assert!(submitters.scope_is_empty(&QuorumScope::Privileged));

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
        let test_data_hash =
            <<Test as frame_system::Config>::Hashing as Hasher>::hash(&test_submission.data);

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
        let submissions = Indexing::submissions(&test_submission.batch_id, test_data_hash);
        assert!(submissions.scope_is_empty(&QuorumScope::Public));
        assert!(submissions.scope_is_empty(&QuorumScope::Privileged));
        assert!(Indexing::final_data(&test_submission.batch_id).is_none());
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
        let submissions = Indexing::submissions(&test_submission.batch_id, test_data_hash);
        assert!(submissions.scope_is_empty(&QuorumScope::Public));
        assert!(submissions.scope_is_empty(&QuorumScope::Privileged));
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
        let submitters = Indexing::submissions(&internal_batch_id, test_data_hash);
        assert!(submitters.scope_is_empty(&QuorumScope::Public));
        assert!(submitters.scope_is_empty(&QuorumScope::Privileged));

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
        let submitters = Indexing::submissions(&internal_batch_id_2, test_data_hash);
        assert!(submitters.scope_is_empty(&QuorumScope::Public));
        assert!(submitters.scope_is_empty(&QuorumScope::Privileged));
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
        let _test_data_hash =
            <<Test as frame_system::Config>::Hashing as Hasher>::hash(&test_submission.data);

        let public_submitter = RuntimeOrigin::signed(sp_runtime::AccountId32::new([1; 32]));
        let _who = ensure_signed(public_submitter.clone()).unwrap();

        // permissionless submission
        assert_ok!(submit_test_data(public_submitter, test_submission.clone()));

        let internal_batch_id =
            build_inner_batch_id::<Test, Api>(&test_submission.batch_id, &table_id);
        assert!(Indexing::final_data(&internal_batch_id).is_some());
    })
}

#[test]
fn set_block_number_works() {
    new_test_ext().execute_with(|| {
        let table_id = TableIdentifier {
            namespace: TableNamespace::try_from(b"TEST_NAMESPACE".to_vec()).unwrap(),
            name: TableName::try_from(b"TEST_TABLE".to_vec()).unwrap(),
        };

        assert_eq!(Indexing::block_numbers(&table_id), None);

        assert_ok!(Indexing::set_block_number(
            RuntimeOrigin::root(),
            table_id.clone(),
            42,
        ));

        assert_eq!(Indexing::block_numbers(&table_id), Some(42));
    });
}

#[test]
fn set_block_number_fails_for_non_sudo() {
    new_test_ext().execute_with(|| {
        let table_id = TableIdentifier {
            namespace: TableNamespace::try_from(b"TEST_NAMESPACE".to_vec()).unwrap(),
            name: TableName::try_from(b"TEST_TABLE".to_vec()).unwrap(),
        };

        let signer = RuntimeOrigin::signed(sp_runtime::AccountId32::new([1; 32]));
        assert_err!(
            Indexing::set_block_number(signer, table_id, 42),
            sp_runtime::DispatchError::BadOrigin,
        );
    });
}

#[test]
fn submit_with_block_enforcement_succeeds_with_increasing_block_numbers() {
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

        // Enable increasing block enforcement
        assert_ok!(Tables::set_block_enforcement(
            RuntimeOrigin::root(),
            table_id.clone(),
            Some(pallet_tables::pallet::BlockEnforcementMode::Increasing),
        ));

        // First insert with block_number 100
        assert_ok!(Indexing::submit_blockchain_data(
            signer.clone(),
            table_id.clone(),
            BatchId::try_from(b"batch_1".to_vec()).unwrap(),
            row_data_with_count(1),
            100,
        ));
        assert_eq!(Indexing::block_numbers(&table_id), Some(100));

        // Second insert with block_number 200 (strictly greater)
        assert_ok!(Indexing::submit_blockchain_data(
            signer,
            table_id.clone(),
            BatchId::try_from(b"batch_2".to_vec()).unwrap(),
            row_data_with_count(1),
            200,
        ));
        assert_eq!(Indexing::block_numbers(&table_id), Some(200));
    });
}

#[test]
fn submit_with_block_enforcement_fails_without_block_number() {
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

        // Enable increasing block enforcement
        assert_ok!(Tables::set_block_enforcement(
            RuntimeOrigin::root(),
            table_id.clone(),
            Some(pallet_tables::pallet::BlockEnforcementMode::Increasing),
        ));

        // Submit via submit_data (no block number) should fail
        assert_err!(
            Indexing::submit_data(
                signer,
                table_id.clone(),
                BatchId::try_from(b"batch_no_bn".to_vec()).unwrap(),
                row_data(),
            ),
            crate::Error::<Test, Api>::BlockNumberRequired,
        );

        // Block number should remain unset
        assert_eq!(Indexing::block_numbers(&table_id), None);
    });
}

#[test]
fn submit_with_block_enforcement_fails_with_non_increasing_block_number() {
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

        // Enable increasing block enforcement
        assert_ok!(Tables::set_block_enforcement(
            RuntimeOrigin::root(),
            table_id.clone(),
            Some(pallet_tables::pallet::BlockEnforcementMode::Increasing),
        ));

        // First insert at block 100
        assert_ok!(Indexing::submit_blockchain_data(
            signer.clone(),
            table_id.clone(),
            BatchId::try_from(b"batch_first".to_vec()).unwrap(),
            row_data_with_count(1),
            100,
        ));

        // Submit with same block number (not strictly greater) should fail
        assert_err!(
            Indexing::submit_blockchain_data(
                signer.clone(),
                table_id.clone(),
                BatchId::try_from(b"batch_equal".to_vec()).unwrap(),
                row_data_with_count(1),
                100,
            ),
            crate::Error::<Test, Api>::BlockNumberNotIncreasing,
        );

        // Block number should remain at 100
        assert_eq!(Indexing::block_numbers(&table_id), Some(100));

        // Submit with lower block number should also fail
        assert_err!(
            Indexing::submit_blockchain_data(
                signer,
                table_id.clone(),
                BatchId::try_from(b"batch_lower".to_vec()).unwrap(),
                row_data_with_count(1),
                50,
            ),
            crate::Error::<Test, Api>::BlockNumberNotIncreasing,
        );

        // Block number should still remain at 100
        assert_eq!(Indexing::block_numbers(&table_id), Some(100));
    });
}

#[test]
fn submit_with_contiguous_enforcement_succeeds_with_sequential_blocks() {
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

        // Enable contiguous block enforcement
        assert_ok!(Tables::set_block_enforcement(
            RuntimeOrigin::root(),
            table_id.clone(),
            Some(pallet_tables::pallet::BlockEnforcementMode::Contiguous),
        ));

        // First insert at block 10
        assert_ok!(Indexing::submit_blockchain_data(
            signer.clone(),
            table_id.clone(),
            BatchId::try_from(b"batch_1".to_vec()).unwrap(),
            row_data_with_count(1),
            10,
        ));
        assert_eq!(Indexing::block_numbers(&table_id), Some(10));

        // Second insert at block 11 (prev + 1)
        assert_ok!(Indexing::submit_blockchain_data(
            signer,
            table_id.clone(),
            BatchId::try_from(b"batch_2".to_vec()).unwrap(),
            row_data_with_count(1),
            11,
        ));
        assert_eq!(Indexing::block_numbers(&table_id), Some(11));
    });
}

#[test]
fn submit_with_contiguous_enforcement_fails_with_non_sequential_blocks() {
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

        // Enable contiguous block enforcement
        assert_ok!(Tables::set_block_enforcement(
            RuntimeOrigin::root(),
            table_id.clone(),
            Some(pallet_tables::pallet::BlockEnforcementMode::Contiguous),
        ));

        // First insert at block 10
        assert_ok!(Indexing::submit_blockchain_data(
            signer.clone(),
            table_id.clone(),
            BatchId::try_from(b"batch_1".to_vec()).unwrap(),
            row_data_with_count(1),
            10,
        ));

        // Skip to block 12 (not prev + 1) should fail
        assert_err!(
            Indexing::submit_blockchain_data(
                signer.clone(),
                table_id.clone(),
                BatchId::try_from(b"batch_skip".to_vec()).unwrap(),
                row_data_with_count(1),
                12,
            ),
            crate::Error::<Test, Api>::BlockNumberNotContiguous,
        );

        // Same block number should also fail
        assert_err!(
            Indexing::submit_blockchain_data(
                signer,
                table_id.clone(),
                BatchId::try_from(b"batch_same".to_vec()).unwrap(),
                row_data_with_count(1),
                10,
            ),
            crate::Error::<Test, Api>::BlockNumberNotContiguous,
        );

        // Block number should remain at 10
        assert_eq!(Indexing::block_numbers(&table_id), Some(10));
    });
}

// ---- submit_empty_blocks tests ----

/// Helper to set up a table with permissions for submit_empty_blocks tests.
fn setup_table_and_permissions() -> (TableIdentifier, RuntimeOrigin) {
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

    (table_id, signer)
}

#[test]
fn submit_empty_blocks_succeeds() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (table_id, signer) = setup_table_and_permissions();

        assert_ok!(Indexing::submit_empty_blocks(
            signer,
            table_id.clone(),
            BatchId::try_from(b"eb_1".to_vec()).unwrap(),
            100,
            105,
        ));

        // Block number should be updated to end_block_number
        assert_eq!(Indexing::block_numbers(&table_id), Some(105));
    });
}

#[test]
fn submit_empty_blocks_fails_with_invalid_range() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (table_id, signer) = setup_table_and_permissions();

        assert_err!(
            Indexing::submit_empty_blocks(
                signer,
                table_id,
                BatchId::try_from(b"eb_bad".to_vec()).unwrap(),
                105,
                100,
            ),
            crate::Error::<Test, Api>::InvalidBlockRange,
        );
    });
}

#[test]
fn submit_empty_blocks_fails_for_unauthorized() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (table_id, _) = setup_table_and_permissions();

        // Use an account without permissions
        let bad_signer = RuntimeOrigin::signed(sp_runtime::AccountId32::new([99; 32]));
        assert_err!(
            Indexing::submit_empty_blocks(
                bad_signer,
                table_id,
                BatchId::try_from(b"eb_unauth".to_vec()).unwrap(),
                1,
                5,
            ),
            crate::Error::<Test, Api>::UnauthorizedSubmitter,
        );
    });
}

#[test]
fn submit_empty_blocks_with_increasing_enforcement_succeeds() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (table_id, signer) = setup_table_and_permissions();

        assert_ok!(Tables::set_block_enforcement(
            RuntimeOrigin::root(),
            table_id.clone(),
            Some(pallet_tables::pallet::BlockEnforcementMode::Increasing),
        ));

        // First empty blocks range
        assert_ok!(Indexing::submit_empty_blocks(
            signer.clone(),
            table_id.clone(),
            BatchId::try_from(b"eb_inc1".to_vec()).unwrap(),
            10,
            20,
        ));
        assert_eq!(Indexing::block_numbers(&table_id), Some(20));

        // Second range starting above previous end
        assert_ok!(Indexing::submit_empty_blocks(
            signer,
            table_id.clone(),
            BatchId::try_from(b"eb_inc2".to_vec()).unwrap(),
            25,
            30,
        ));
        assert_eq!(Indexing::block_numbers(&table_id), Some(30));
    });
}

#[test]
fn submit_empty_blocks_with_increasing_enforcement_fails_non_increasing() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (table_id, signer) = setup_table_and_permissions();

        assert_ok!(Tables::set_block_enforcement(
            RuntimeOrigin::root(),
            table_id.clone(),
            Some(pallet_tables::pallet::BlockEnforcementMode::Increasing),
        ));

        assert_ok!(Indexing::submit_empty_blocks(
            signer.clone(),
            table_id.clone(),
            BatchId::try_from(b"eb_first".to_vec()).unwrap(),
            10,
            20,
        ));

        // start_block_number not greater than previous end (20)
        assert_err!(
            Indexing::submit_empty_blocks(
                signer,
                table_id.clone(),
                BatchId::try_from(b"eb_bad_inc".to_vec()).unwrap(),
                15,
                25,
            ),
            crate::Error::<Test, Api>::BlockNumberNotIncreasing,
        );

        assert_eq!(Indexing::block_numbers(&table_id), Some(20));
    });
}

#[test]
fn submit_empty_blocks_with_contiguous_enforcement_succeeds() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (table_id, signer) = setup_table_and_permissions();

        assert_ok!(Tables::set_block_enforcement(
            RuntimeOrigin::root(),
            table_id.clone(),
            Some(pallet_tables::pallet::BlockEnforcementMode::Contiguous),
        ));

        assert_ok!(Indexing::submit_empty_blocks(
            signer.clone(),
            table_id.clone(),
            BatchId::try_from(b"eb_c1".to_vec()).unwrap(),
            10,
            15,
        ));
        assert_eq!(Indexing::block_numbers(&table_id), Some(15));

        // Next range starts at prev + 1 = 16
        assert_ok!(Indexing::submit_empty_blocks(
            signer,
            table_id.clone(),
            BatchId::try_from(b"eb_c2".to_vec()).unwrap(),
            16,
            20,
        ));
        assert_eq!(Indexing::block_numbers(&table_id), Some(20));
    });
}

#[test]
fn submit_empty_blocks_with_contiguous_enforcement_fails_gap() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (table_id, signer) = setup_table_and_permissions();

        assert_ok!(Tables::set_block_enforcement(
            RuntimeOrigin::root(),
            table_id.clone(),
            Some(pallet_tables::pallet::BlockEnforcementMode::Contiguous),
        ));

        assert_ok!(Indexing::submit_empty_blocks(
            signer.clone(),
            table_id.clone(),
            BatchId::try_from(b"eb_g1".to_vec()).unwrap(),
            10,
            15,
        ));

        // Gap: expected 16 but got 17
        assert_err!(
            Indexing::submit_empty_blocks(
                signer,
                table_id.clone(),
                BatchId::try_from(b"eb_gap".to_vec()).unwrap(),
                17,
                20,
            ),
            crate::Error::<Test, Api>::BlockNumberNotContiguous,
        );

        assert_eq!(Indexing::block_numbers(&table_id), Some(15));
    });
}

#[test]
fn submit_empty_blocks_with_contiguous_enforcement_fails_non_increasing() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (table_id, signer) = setup_table_and_permissions();

        assert_ok!(Tables::set_block_enforcement(
            RuntimeOrigin::root(),
            table_id.clone(),
            Some(pallet_tables::pallet::BlockEnforcementMode::Contiguous),
        ));

        // First range: [10, 20] succeeds, BlockNumbers = Some(20)
        assert_ok!(Indexing::submit_empty_blocks(
            signer.clone(),
            table_id.clone(),
            BatchId::try_from(b"eb_c_first".to_vec()).unwrap(),
            10,
            20,
        ));
        assert_eq!(Indexing::block_numbers(&table_id), Some(20));

        // Second range [15, 25]: start (15) < prev_end (20), so 15 != 20 + 1 = 21
        assert_err!(
            Indexing::submit_empty_blocks(
                signer,
                table_id.clone(),
                BatchId::try_from(b"eb_c_overlap".to_vec()).unwrap(),
                15,
                25,
            ),
            crate::Error::<Test, Api>::BlockNumberNotContiguous,
        );

        // BlockNumbers unchanged
        assert_eq!(Indexing::block_numbers(&table_id), Some(20));
    });
}

#[test]
fn submit_empty_blocks_batch_id_is_scoped_to_table() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        // Create a shared namespace for two tables
        let namespace = TableNamespace::try_from(b"SCOPE_TEST".to_vec()).unwrap();
        Tables::create_namespace(
            RuntimeOrigin::root(),
            namespace.clone(),
            0,
            b"CREATE SCHEMA IF NOT EXISTS SCOPE_TEST"
                .to_vec()
                .try_into()
                .unwrap(),
            TableType::CoreBlockchain,
            sxt_core::tables::Source::Ethereum,
        )
        .unwrap();

        let table_a = TableIdentifier {
            namespace: namespace.clone(),
            name: TableName::try_from(b"TABLE_A".to_vec()).unwrap(),
        };
        let table_b = TableIdentifier {
            namespace: namespace.clone(),
            name: TableName::try_from(b"TABLE_B".to_vec()).unwrap(),
        };

        let table_type = TableType::Testing(InsertQuorumSize {
            public: Some(0),
            privileged: None,
        });

        for (table, suffix) in [(&table_a, "TABLE_A"), (&table_b, "TABLE_B")] {
            Tables::create_tables(
                RuntimeOrigin::root(),
                vec![UpdateTable {
                    ident: table.clone(),
                    create_statement: CreateStatement::try_from(
                        format!("CREATE TABLE SCOPE_TEST.{suffix} (int_column INT NOT NULL)")
                            .into_bytes(),
                    )
                    .unwrap(),
                    table_type: table_type.clone(),
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
        }

        let signer = RuntimeOrigin::signed(sp_runtime::AccountId32::new([42; 32]));
        let who = ensure_signed(signer.clone()).unwrap();
        pallet_permissions::Permissions::<Test>::insert(
            who,
            PermissionList::try_from(vec![PermissionLevel::IndexingPallet(
                IndexingPalletPermission::SubmitDataForPublicQuorum,
            )])
            .unwrap(),
        );

        // The same outer batch_id is reused for both tables
        let outer_batch_id = BatchId::try_from(b"shared_batch".to_vec()).unwrap();

        // Inner batch ids must differ because they are hashed with the table identifier
        let inner_a = build_inner_batch_id::<Test, Api>(&outer_batch_id, &table_a);
        let inner_b = build_inner_batch_id::<Test, Api>(&outer_batch_id, &table_b);
        assert_ne!(
            inner_a, inner_b,
            "inner batch IDs must differ across tables"
        );

        // Submit to table_a with quorum=0 — finalizes immediately
        assert_ok!(Indexing::submit_empty_blocks(
            signer.clone(),
            table_a.clone(),
            outer_batch_id.clone(),
            10,
            20,
        ));
        assert_eq!(Indexing::block_numbers(&table_a), Some(20));
        assert!(Indexing::final_data(&inner_a).is_some());

        // table_b must be unaffected
        assert_eq!(Indexing::block_numbers(&table_b), None);
        assert!(Indexing::final_data(&inner_b).is_none());

        // Submit to table_b with the same outer batch_id — must also succeed independently
        assert_ok!(Indexing::submit_empty_blocks(
            signer,
            table_b.clone(),
            outer_batch_id.clone(),
            10,
            20,
        ));
        assert_eq!(Indexing::block_numbers(&table_b), Some(20));
        assert!(Indexing::final_data(&inner_b).is_some());

        // Each FinalData entry is keyed to its own table
        assert_eq!(Indexing::final_data(&inner_a).unwrap().table, table_a);
        assert_eq!(Indexing::final_data(&inner_b).unwrap().table, table_b);
    });
}

#[test]
fn submit_empty_blocks_respects_quorum() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (table_id, create_stmt) = sample_table_definition();

        // Require 2 out of 3 submissions (quorum_size = 1 means >1 i.e. 2+ needed)
        Tables::create_tables(
            RuntimeOrigin::root(),
            vec![UpdateTable {
                ident: table_id.clone(),
                create_statement: create_stmt,
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

        let permissions = PermissionList::try_from(vec![PermissionLevel::IndexingPallet(
            IndexingPalletPermission::SubmitDataForPublicQuorum,
        )])
        .unwrap();
        for seed in [1u8, 2, 3] {
            let who = ensure_signed(RuntimeOrigin::signed(sp_runtime::AccountId32::new(
                [seed; 32],
            )))
            .unwrap();
            pallet_permissions::Permissions::<Test>::insert(who, permissions.clone());
        }

        let batch_id = BatchId::try_from(b"quorum_eb".to_vec()).unwrap();
        let signer1 = RuntimeOrigin::signed(sp_runtime::AccountId32::new([1; 32]));
        let signer2 = RuntimeOrigin::signed(sp_runtime::AccountId32::new([2; 32]));
        let signer3 = RuntimeOrigin::signed(sp_runtime::AccountId32::new([3; 32]));

        // First submission — quorum not yet reached, block number not updated
        assert_ok!(Indexing::submit_empty_blocks(
            signer1,
            table_id.clone(),
            batch_id.clone(),
            10,
            20,
        ));
        assert_eq!(Indexing::block_numbers(&table_id), None);

        // Second submission — quorum reached, block number updated to end_block_number
        assert_ok!(Indexing::submit_empty_blocks(
            signer2,
            table_id.clone(),
            batch_id.clone(),
            10,
            20,
        ));
        assert_eq!(Indexing::block_numbers(&table_id), Some(20));

        // Third submission — batch already finalized
        assert_err!(
            Indexing::submit_empty_blocks(signer3, table_id.clone(), batch_id.clone(), 10, 20,),
            crate::Error::<Test, Api>::LateBatch,
        );
    });
}
