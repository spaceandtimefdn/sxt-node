use alloc::boxed::Box;
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
use frame_support::{assert_err, assert_ok};
use frame_system::ensure_signed;
use native_api::Api;
use pallet_tables::{CommitmentCreationCmd, UpdateTable};
use proof_of_sql_commitment_map::CommitmentSchemeFlags;
use sp_core::Hasher;
use sp_runtime::BoundedVec;
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
use crate::{BatchId, Event, RowData};

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

fn row_data() -> RowData {
    let schema = Arc::new(Schema::new(vec![Field::new(
        "int_column",
        DataType::Int32,
        false,
    )]));

    let int_data = Arc::new(Int32Array::from(vec![1, 2, 3, 4])) as ArrayRef;

    let batch = RecordBatch::try_new(schema.clone(), vec![int_data]).unwrap();

    record_batch_to_row_data(batch, schema)
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
        b"CREATE TABLE test_namespace.test_table (int_column INT NOT NULL)"
            .to_owned()
            .to_vec(),
    )
    .unwrap();

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
        b"CREATE TABLE test_namespace.test_table (
            int_column INT NOT NULL,
            block_number BIGINT NOT NULL
        )"
        .to_vec(),
    )
    .unwrap();

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

        let signer = RuntimeOrigin::signed(1);
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

        // Verify that the submission was stored as expected
        // and the hash was generated from the submitted data
        assert_eq!(
            Indexing::submissions(test_batch.clone(), hash).len_of_scope(&QuorumScope::Public),
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
        let signer = RuntimeOrigin::signed(1);
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

        // Verify that the submission was stored as expected
        // and the hash was generated from the submitted data
        assert_eq!(
            Indexing::submissions(test_batch.clone(), hash).len_of_scope(&QuorumScope::Public),
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
        let signer = RuntimeOrigin::signed(1);
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
            table: table_id,
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
            let who = ensure_signed(RuntimeOrigin::signed(id)).unwrap();
            pallet_permissions::Permissions::<Test>::insert(who, permissions.clone());
        }

        // Submit 4 entries with 4 different accounts
        assert_ok!(submit_test_data(
            RuntimeOrigin::signed(1),
            test_submission.clone()
        ));
        assert_ok!(submit_test_data(
            RuntimeOrigin::signed(2),
            test_submission.clone()
        ));
        assert_ok!(submit_test_data(
            RuntimeOrigin::signed(3),
            test_submission.clone()
        ));

        // We haven't reached enough submissions yet, so this should not be decided on
        assert!(Indexing::final_data(test_submission.batch_id.clone()).is_none());

        // Send the final required submission
        assert_ok!(submit_test_data(
            RuntimeOrigin::signed(4),
            test_submission.clone()
        ));

        // Now that we have 4 submissions, verify that the data was decided on
        let maybe_final_data = Indexing::final_data(test_submission.batch_id.clone());
        assert!(maybe_final_data.is_some());

        let fd = maybe_final_data.unwrap();
        assert_eq!(fd.data_hash, test_data_hash);
        assert_eq!(fd.table, test_submission.table);
        assert_eq!(fd.quorum_scope, QuorumScope::Public);

        // Verify that the old data was successfully removed for this batch
        let submitters = Indexing::submissions(test_submission.batch_id.clone(), test_data_hash);
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
            let who = ensure_signed(RuntimeOrigin::signed(id)).unwrap();
            let permissions = PermissionList::try_from(vec![PermissionLevel::IndexingPallet(
                IndexingPalletPermission::SubmitDataForPublicQuorum,
            )])
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

        // Submit 4 entries with 4 different accounts
        assert_ok!(submit_test_data(
            RuntimeOrigin::signed(1),
            test_submission.clone()
        ));
        assert_ok!(submit_test_data(
            RuntimeOrigin::signed(2),
            test_submission.clone()
        ));
        assert_ok!(submit_test_data(
            RuntimeOrigin::signed(3),
            test_submission.clone()
        ));

        // We haven't reached enough submissions yet, so this should not be decided on
        assert!(Indexing::final_data(test_submission.batch_id.clone()).is_none());

        // Send a submission that is with different data
        let differing_submission = TestSubmission {
            table: table_id,
            batch_id: test_batch_id,
            data: diff_row_data(),
        };
        assert_ok!(submit_test_data(
            RuntimeOrigin::signed(4),
            differing_submission.clone()
        ));

        // This should still not be decided on yet, so double check
        assert!(Indexing::final_data(test_submission.batch_id.clone()).is_none());

        // Now submit a final matching entry
        assert_ok!(submit_test_data(
            RuntimeOrigin::signed(5),
            test_submission.clone()
        ));

        // Now that we have 4 submissions, verify that the data was decided on
        let final_data = Indexing::final_data(test_submission.batch_id.clone());
        assert!(final_data.is_some());

        // Verify that it matches the originally submitted test data
        assert_eq!(final_data.unwrap().data_hash, data_hash);

        // Verify that the old data was successfully removed for this batch
        for _i in 1..4 {
            assert!(
                Indexing::submissions(test_submission.batch_id.clone(), data_hash)
                    .scope_is_empty(&QuorumScope::Public)
            )
        }
    })
}

#[test]
fn inserting_data_fails_when_data_is_empty() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let signer = RuntimeOrigin::signed(1);
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

        let signer = RuntimeOrigin::signed(1);
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
fn inserting_data_fails_when_table_namespace_is_empty() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let signer = RuntimeOrigin::signed(1);
        let who = ensure_signed(signer.clone()).unwrap();
        let permissions = PermissionList::try_from(vec![PermissionLevel::IndexingPallet(
            IndexingPalletPermission::SubmitDataForPublicQuorum,
        )])
        .unwrap();
        pallet_permissions::Permissions::<Test>::insert(who, permissions.clone());

        let (table_id, create_statement) = sample_table_definition();
        let test_identifier = TableIdentifier {
            // Create an empty namespace
            namespace: TableNamespace::try_from(b"".to_vec()).unwrap(),
            ..table_id
        };
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
fn inserting_data_fails_when_batch_id_is_empty() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let signer = RuntimeOrigin::signed(1);
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
            let who = ensure_signed(RuntimeOrigin::signed(id)).unwrap();
            pallet_permissions::Permissions::<Test>::insert(who, permissions.clone());
        }

        let test_batch_id = BatchId::try_from(b"test_batch".to_vec()).unwrap();
        let test_submission = TestSubmission {
            table: table_id,
            batch_id: test_batch_id.clone(),
            data: row_data(),
        };
        let data_hash = hash_row_data_with_block_number::<Test>(&test_submission.data, None);

        // Submit enough data to ensure the quorum is reached
        for i in 0..4 {
            assert_ok!(Indexing::submit_data(
                RuntimeOrigin::signed(i),
                test_submission.table.clone(),
                test_submission.batch_id.clone(),
                test_submission.data.clone()
            ));
        }

        // Verify that the data is finalized
        let maybe_data = Indexing::final_data(test_submission.batch_id.clone());
        assert!(maybe_data.is_some());
        let quorum = maybe_data.unwrap();
        assert_eq!(quorum.data_hash, data_hash);
        assert_eq!(quorum.table, test_submission.table);

        // Future submissions to this batch should receive the LateBatch Error
        let who = ensure_signed(RuntimeOrigin::signed(1234)).unwrap();
        pallet_permissions::Permissions::<Test>::insert(who, permissions.clone());
        assert_err!(
            Indexing::submit_data(
                RuntimeOrigin::signed(1234),
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

        let signer_key = 1;
        let signer = RuntimeOrigin::signed(signer_key);
        let admin = 2;

        let admin_permission = PermissionLevel::EditSpecificPermission(Box::new(
            PermissionLevel::IndexingPallet(IndexingPalletPermission::SubmitDataForPublicQuorum),
        ));
        let permission_list = BoundedVec::try_from(vec![admin_permission]).unwrap();
        assert_ok!(pallet_permissions::Pallet::<Test>::set_permissions(
            RuntimeOrigin::root(),
            admin,
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

        // Verify that the submission was stored as expected
        // and the hash was generated from the submitted data
        assert_eq!(
            Indexing::submissions(test_batch.clone(), hash).len_of_scope(&QuorumScope::Public),
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
            IndexingPalletPermission::SubmitDataForPrivilegedQuorum(table_id),
        )])
        .unwrap();

        let origin = RuntimeOrigin::signed(1);
        let who = ensure_signed(origin.clone()).unwrap();
        pallet_permissions::Permissions::<Test>::insert(who, permissions.clone());

        // Send the final required submission
        assert_ok!(submit_test_data(origin, test_submission.clone()));

        let maybe_final_data = Indexing::final_data(test_submission.batch_id.clone());
        assert!(maybe_final_data.is_some());

        let fd = maybe_final_data.unwrap();
        assert_eq!(fd.data_hash, test_data_hash);
        assert_eq!(fd.table, test_submission.table);
        assert_eq!(fd.quorum_scope, QuorumScope::Privileged);

        // Verify that the old data was successfully removed for this batch
        let submitters = Indexing::submissions(test_submission.batch_id.clone(), test_data_hash);
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
            IndexingPalletPermission::SubmitDataForPrivilegedQuorum(table_id),
        );

        let public_submitter = RuntimeOrigin::signed(1);
        let who = ensure_signed(public_submitter.clone()).unwrap();
        pallet_permissions::Permissions::<Test>::insert(
            who,
            PermissionList::try_from(vec![public_permission.clone()]).unwrap(),
        );

        let privileged_submitter = RuntimeOrigin::signed(2);
        let who = ensure_signed(privileged_submitter.clone()).unwrap();
        pallet_permissions::Permissions::<Test>::insert(
            who,
            PermissionList::try_from(vec![privileged_permission.clone()]).unwrap(),
        );

        let both_submitter = RuntimeOrigin::signed(3);
        let who = ensure_signed(both_submitter.clone()).unwrap();
        pallet_permissions::Permissions::<Test>::insert(
            who,
            PermissionList::try_from(vec![public_permission, privileged_permission]).unwrap(),
        );

        // public submission
        assert_ok!(submit_test_data(public_submitter, test_submission.clone()));
        let submissions = Indexing::submissions(&test_submission.batch_id, test_data_hash);
        assert_eq!(submissions.len_of_scope(&QuorumScope::Public), 1);
        assert!(submissions.scope_is_empty(&QuorumScope::Privileged));
        assert!(Indexing::final_data(&test_submission.batch_id).is_none());

        // both submission
        assert_ok!(submit_test_data(both_submitter, test_submission.clone()));
        let submissions = Indexing::submissions(&test_submission.batch_id, test_data_hash);
        assert_eq!(submissions.len_of_scope(&QuorumScope::Public), 2);
        assert_eq!(submissions.len_of_scope(&QuorumScope::Privileged), 1);
        assert!(Indexing::final_data(&test_submission.batch_id).is_none());

        // privileged submission
        assert_ok!(submit_test_data(
            privileged_submitter,
            test_submission.clone()
        ));
        let final_data = Indexing::final_data(&test_submission.batch_id).unwrap();

        assert_eq!(final_data.data_hash, test_data_hash);
        assert_eq!(final_data.table, test_submission.table);
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
fn reaching_quorum_for_both_scopes_simultaneously_produces_one_quorum_reached_event() {
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
            IndexingPalletPermission::SubmitDataForPrivilegedQuorum(table_id),
        );

        let both_submitter = RuntimeOrigin::signed(3);
        let who = ensure_signed(both_submitter.clone()).unwrap();
        pallet_permissions::Permissions::<Test>::insert(
            who,
            PermissionList::try_from(vec![public_permission, privileged_permission]).unwrap(),
        );

        // both submission
        assert_ok!(submit_test_data(both_submitter, test_submission.clone()));

        let final_data = Indexing::final_data(&test_submission.batch_id).unwrap();

        assert_eq!(final_data.data_hash, test_data_hash);
        assert_eq!(final_data.table, test_submission.table);
        // Public quorum is selected over privileged in this case
        assert_eq!(final_data.quorum_scope, QuorumScope::Public);

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

        let public_submitter = RuntimeOrigin::signed(1);
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

        let privileged_submitter = RuntimeOrigin::signed(1);
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

        let signer = RuntimeOrigin::signed(1);
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

        let signer = RuntimeOrigin::signed(1);
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

        let signer = RuntimeOrigin::signed(1);
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
            RuntimeOrigin::signed(1),
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

        let signer = RuntimeOrigin::signed(1);
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
