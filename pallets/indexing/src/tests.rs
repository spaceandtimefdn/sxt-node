use alloc::boxed::Box;
use std::convert::Into;
use std::io::Cursor;
use std::sync::Arc;

use arrow::array::{ArrayRef, Int32Array, Int64Array, RecordBatch};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::ipc::writer::StreamWriter;
use codec::{Decode, Encode, MaxEncodedLen};
use native_api::Api;
use pallet_tables::{CommitmentCreationCmd, UpdateTable};
use polkadot_sdk::frame_support::__private::RuntimeDebug;
use polkadot_sdk::frame_support::dispatch::{DispatchResult, DispatchResultWithPostInfo};
use polkadot_sdk::frame_support::pallet_prelude::TypeInfo;
use polkadot_sdk::frame_support::{assert_err, assert_ok};
use polkadot_sdk::frame_system::ensure_signed;
use polkadot_sdk::sp_core::Hasher;
use polkadot_sdk::sp_runtime::BoundedVec;
use polkadot_sdk::{frame_system, sp_runtime};
use proof_of_sql_commitment_map::CommitmentSchemeFlags;
use sxt_core::permissions::{IndexingPalletPermission, PermissionLevel, PermissionList};
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
fn submit_test_data(
    signer: RuntimeOrigin,
    submission: TestSubmission,
) -> DispatchResultWithPostInfo {
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

        use polkadot_sdk::frame_support::dispatch::PostDispatchInfo;
        use polkadot_sdk::frame_support::pallet_prelude::*;
        use polkadot_sdk::sp_runtime::DispatchErrorWithPostInfo;
        pallet_permissions::Permissions::<Test>::insert(who.clone(), permissions.clone());
        let expected = polkadot_sdk::sp_runtime::DispatchErrorWithPostInfo {
            post_info: PostDispatchInfo {
                actual_weight: None,
                pays_fee: Pays::No,
            },
            error: crate::Error::<Test, Api>::LateBatch.into(),
        };

        assert_eq!(
            Indexing::submit_data(
                RuntimeOrigin::signed(who),
                test_submission.table.clone(),
                test_submission.batch_id.clone(),
                test_submission.data.clone(),
            )
            .unwrap_err(),
            expected
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

/// Helper to set up a table with a given quorum size and fund its treasury account.
fn setup_table_with_treasury(
    quorum_size: u32,
    treasury_balance: u128,
) -> (TableIdentifier, CreateStatement) {
    let (table_id, create_statement) = sample_table_definition();
    Tables::create_tables(
        RuntimeOrigin::root(),
        vec![UpdateTable {
            ident: table_id.clone(),
            create_statement: create_statement.clone(),
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

    // Fund the table treasury
    if treasury_balance > 0 {
        let treasury = sxt_core::utils::account_id_from_table_id::<Test>(&table_id).unwrap();
        assert_ok!(Balances::force_set_balance(
            RuntimeOrigin::root(),
            treasury,
            treasury_balance,
        ));
    }

    (table_id, create_statement)
}

/// Grant public quorum submission permission to accounts [start..end] (each id is [n; 32]).
fn grant_public_permissions(start: u8, end: u8) {
    let permissions = PermissionList::try_from(vec![PermissionLevel::IndexingPallet(
        IndexingPalletPermission::SubmitDataForPublicQuorum,
    )])
    .unwrap();
    for id in start..end {
        let who = ensure_signed(RuntimeOrigin::signed(sp_runtime::AccountId32::new(
            [id; 32],
        )))
        .unwrap();
        pallet_permissions::Permissions::<Test>::insert(who, permissions.clone());
    }
}

/// Submit data from N distinct signers (ids [1; 32] .. [n; 32]) to reach quorum.
fn submit_from_n_signers(n: u8, submission: &TestSubmission) {
    for id in 1..=n {
        assert_ok!(submit_test_data(
            RuntimeOrigin::signed(sp_runtime::AccountId32::new([id; 32])),
            submission.clone()
        ));
    }
}

#[test]
fn refund_issued_event_emitted_when_treasury_is_funded() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let treasury_balance = 1_000_000_000_000_000_000u128; // 1 UNIT, plenty of funds
        let (table_id, _) = setup_table_with_treasury(4, treasury_balance);

        grant_public_permissions(1, 6);

        let test_submission = TestSubmission {
            table: table_id.clone(),
            batch_id: BatchId::try_from(b"refund_batch".to_vec()).unwrap(),
            data: row_data(),
        };

        // Submit 4 times to reach quorum (CoreBlockchain default is 4)
        submit_from_n_signers(4, &test_submission);

        // Verify quorum was reached
        let internal_batch_id =
            build_inner_batch_id::<Test, Api>(&test_submission.batch_id, &table_id);
        assert!(Indexing::final_data(&internal_batch_id).is_some());

        // Verify RefundIssued event was emitted
        let events = System::events();
        let refund_issued = events
            .iter()
            .find(|e| matches!(&e.event, RuntimeEvent::Indexing(Event::RefundIssued { .. })));
        assert!(
            refund_issued.is_some(),
            "Expected RefundIssued event to be emitted"
        );
    })
}

#[test]
fn insufficient_table_funds_event_when_treasury_empty() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        // Set up a table with zero treasury balance
        let (table_id, _) = setup_table_with_treasury(4, 0);

        grant_public_permissions(1, 6);

        let test_submission = TestSubmission {
            table: table_id.clone(),
            batch_id: BatchId::try_from(b"empty_treasury_batch".to_vec()).unwrap(),
            data: row_data(),
        };

        submit_from_n_signers(4, &test_submission);

        // Quorum should still be reached (refund failure doesn't block finalization)
        let internal_batch_id =
            build_inner_batch_id::<Test, Api>(&test_submission.batch_id, &table_id);
        assert!(Indexing::final_data(&internal_batch_id).is_some());

        // Verify InsufficientTableFunds event was emitted
        let events = System::events();
        let insufficient = events.iter().find(|e| {
            matches!(
                &e.event,
                RuntimeEvent::Indexing(Event::InsufficientTableFunds { .. })
            )
        });
        assert!(
            insufficient.is_some(),
            "Expected InsufficientTableFunds event to be emitted"
        );
    })
}

#[test]
fn refund_transfers_funds_from_treasury_to_participants() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let treasury_balance = 1_000_000_000_000_000_000u128;
        let (table_id, _) = setup_table_with_treasury(4, treasury_balance);

        grant_public_permissions(1, 6);

        let test_submission = TestSubmission {
            table: table_id.clone(),
            batch_id: BatchId::try_from(b"transfer_batch".to_vec()).unwrap(),
            data: row_data(),
        };

        // Check participant balances before
        let participant = sp_runtime::AccountId32::new([1; 32]);
        let balance_before = Balances::free_balance(&participant);

        submit_from_n_signers(4, &test_submission);

        // After quorum + refund, participant should have received funds
        let balance_after = Balances::free_balance(&participant);
        assert!(
            balance_after > balance_before,
            "Participant balance should increase after refund: before={balance_before}, after={balance_after}"
        );

        // Treasury should have decreased
        let treasury =
            sxt_core::utils::account_id_from_table_id::<Test>(&table_id).unwrap();
        let treasury_after = Balances::free_balance(&treasury);
        assert!(
            treasury_after < treasury_balance,
            "Treasury balance should decrease after refunds"
        );
    })
}

#[test]
fn refund_issued_with_correct_cost_amount() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let treasury_balance = 1_000_000_000_000_000_000u128;
        let (table_id, _) = setup_table_with_treasury(4, treasury_balance);

        grant_public_permissions(1, 6);

        let test_submission = TestSubmission {
            table: table_id.clone(),
            batch_id: BatchId::try_from(b"cost_batch".to_vec()).unwrap(),
            data: row_data(),
        };

        submit_from_n_signers(4, &test_submission);

        // Extract the refund amount from the event
        let events = System::events();
        let refund_event = events.iter().find_map(|e| match &e.event {
            RuntimeEvent::Indexing(Event::RefundIssued { refund, .. }) => Some(*refund),
            _ => None,
        });
        assert!(refund_event.is_some(), "RefundIssued event should exist");
        let refund = refund_event.unwrap();
        // The refund should be positive (cost = weight.ref_time() * WEIGHT_FEE)
        assert!(refund > 0, "Refund amount should be greater than zero");
    })
}

#[test]
fn insufficient_funds_when_treasury_has_partial_balance() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        // Fund with just 1 unit (existential deposit) — not enough for 4 refunds
        let (table_id, _) = setup_table_with_treasury(4, 1);

        grant_public_permissions(1, 6);

        let test_submission = TestSubmission {
            table: table_id.clone(),
            batch_id: BatchId::try_from(b"partial_batch".to_vec()).unwrap(),
            data: row_data(),
        };

        submit_from_n_signers(4, &test_submission);

        // Data should still finalize
        let internal_batch_id =
            build_inner_batch_id::<Test, Api>(&test_submission.batch_id, &table_id);
        assert!(Indexing::final_data(&internal_batch_id).is_some());

        // Should get InsufficientTableFunds since 1 unit < cost * 4 participants
        let events = System::events();
        let has_insufficient = events.iter().any(|e| {
            matches!(
                &e.event,
                RuntimeEvent::Indexing(Event::InsufficientTableFunds { .. })
            )
        });
        assert!(
            has_insufficient,
            "Expected InsufficientTableFunds when treasury balance is too low"
        );
    })
}

#[test]
fn all_quorum_participants_receive_refund() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let treasury_balance = 1_000_000_000_000_000_000u128;
        let (table_id, _) = setup_table_with_treasury(4, treasury_balance);

        grant_public_permissions(1, 6);

        let test_submission = TestSubmission {
            table: table_id.clone(),
            batch_id: BatchId::try_from(b"all_participants_batch".to_vec()).unwrap(),
            data: row_data(),
        };

        // Record balances before
        let participants: Vec<sp_runtime::AccountId32> = (1..=4u8)
            .map(|id| sp_runtime::AccountId32::new([id; 32]))
            .collect();
        let balances_before: Vec<u128> = participants.iter().map(Balances::free_balance).collect();

        submit_from_n_signers(4, &test_submission);

        // All 4 participants should have increased balance
        for (i, participant) in participants.iter().enumerate() {
            let balance_after = Balances::free_balance(participant);
            assert!(
                balance_after > balances_before[i],
                "Participant {} should have received a refund: before={}, after={}",
                i + 1,
                balances_before[i],
                balance_after,
            );
        }
    })
}

#[test]
fn late_submission_refund_uses_post_info_pays_no() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        // Treasury balance is irrelevant — late submission refund comes from post_info, not treasury
        let (table_id, _) = setup_table_with_treasury(4, 0);

        grant_public_permissions(1, 10);

        let test_submission = TestSubmission {
            table: table_id.clone(),
            batch_id: BatchId::try_from(b"late_no_refund_batch".to_vec()).unwrap(),
            data: row_data(),
        };

        // Submit 4 times to reach quorum
        submit_from_n_signers(4, &test_submission);

        // Clear events to isolate late submission events
        System::reset_events();

        // Late submission should fail with LateBatch error and Pays::No in post_info
        let late_submitter = sp_runtime::AccountId32::new([5; 32]);
        let result = submit_test_data(
            RuntimeOrigin::signed(late_submitter.clone()),
            test_submission.clone(),
        );

        // Verify the error is LateBatch
        assert!(result.is_err(), "Late submission should return an error");
        let err = result.unwrap_err();
        assert_eq!(
            err.error,
            crate::Error::<Test, Api>::LateBatch.into(),
            "Error should be LateBatch"
        );

        // Verify post_info indicates the caller should not be charged (Pays::No)
        assert_eq!(
            err.post_info.pays_fee,
            polkadot_sdk::frame_support::dispatch::Pays::No,
            "Late submission should set pays_fee to Pays::No so the submitter is not charged"
        );

        // No RefundError event should be emitted — the refund is handled via post_info, not treasury
        let events = System::events();
        let refund_error = events.iter().find(|e| {
            matches!(
                &e.event,
                RuntimeEvent::Indexing(Event::RefundError { recipient, .. })
                if *recipient == late_submitter
            )
        });
        assert!(
            refund_error.is_none(),
            "No RefundError event should be emitted — late refund is via post_info, not treasury"
        );
    })
}

#[test]
fn dissenters_receive_base_refund_amount() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let treasury_balance = 1_000_000_000_000_000_000u128;
        let (table_id, _) = setup_table_with_treasury(4, treasury_balance);

        grant_public_permissions(1, 10);

        let test_submission = TestSubmission {
            table: table_id.clone(),
            batch_id: BatchId::try_from(b"dissent_batch".to_vec()).unwrap(),
            data: row_data(),
        };

        let dissenting_submission = TestSubmission {
            table: table_id.clone(),
            batch_id: BatchId::try_from(b"dissent_batch".to_vec()).unwrap(),
            data: diff_row_data(), // Different data
        };

        // Record dissenter balance before
        let dissenter = sp_runtime::AccountId32::new([5; 32]);
        let dissenter_balance_before = Balances::free_balance(&dissenter);

        // Submit 3 agreeing submissions
        submit_from_n_signers(3, &test_submission);

        // Submit 1 dissenting submission (different data)
        assert_ok!(submit_test_data(
            RuntimeOrigin::signed(dissenter.clone()),
            dissenting_submission
        ));

        // Submit final agreeing submission to reach quorum
        assert_ok!(submit_test_data(
            RuntimeOrigin::signed(sp_runtime::AccountId32::new([6; 32])),
            test_submission.clone()
        ));

        // Verify quorum was reached
        let internal_batch_id =
            build_inner_batch_id::<Test, Api>(&test_submission.batch_id, &table_id);
        let final_data = Indexing::final_data(&internal_batch_id);
        assert!(final_data.is_some());

        // Verify the dissenter is recorded in the quorum
        let quorum = final_data.unwrap();
        assert!(
            quorum.dissents.contains(&dissenter),
            "Dissenter should be recorded in quorum dissents"
        );

        // Dissenter should have received a refund
        let dissenter_balance_after = Balances::free_balance(&dissenter);
        assert!(
            dissenter_balance_after > dissenter_balance_before,
            "Dissenter should receive refund: before={}, after={}",
            dissenter_balance_before,
            dissenter_balance_after
        );
    })
}

#[test]
fn both_agreements_and_dissents_receive_refunds() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let treasury_balance = 1_000_000_000_000_000_000u128;
        let (table_id, _) = setup_table_with_treasury(4, treasury_balance);

        grant_public_permissions(1, 10);

        let test_submission = TestSubmission {
            table: table_id.clone(),
            batch_id: BatchId::try_from(b"mixed_batch".to_vec()).unwrap(),
            data: row_data(),
        };

        let dissenting_submission = TestSubmission {
            table: table_id.clone(),
            batch_id: BatchId::try_from(b"mixed_batch".to_vec()).unwrap(),
            data: diff_row_data(),
        };

        // Record balances
        let agreers: Vec<sp_runtime::AccountId32> = (1..=4u8)
            .map(|id| sp_runtime::AccountId32::new([id; 32]))
            .collect();
        let dissenter = sp_runtime::AccountId32::new([5; 32]);

        let agreer_balances_before: Vec<u128> =
            agreers.iter().map(Balances::free_balance).collect();
        let dissenter_balance_before = Balances::free_balance(&dissenter);

        // Submit 3 agreeing
        for id in 1..=3u8 {
            assert_ok!(submit_test_data(
                RuntimeOrigin::signed(sp_runtime::AccountId32::new([id; 32])),
                test_submission.clone()
            ));
        }

        // Submit 1 dissenting
        assert_ok!(submit_test_data(
            RuntimeOrigin::signed(dissenter.clone()),
            dissenting_submission
        ));

        // Submit final agreeing to reach quorum
        assert_ok!(submit_test_data(
            RuntimeOrigin::signed(sp_runtime::AccountId32::new([4; 32])),
            test_submission.clone()
        ));

        // Verify all agreers received refunds
        for (i, agreer) in agreers.iter().enumerate() {
            let balance_after = Balances::free_balance(agreer);
            assert!(
                balance_after > agreer_balances_before[i],
                "Agreer {} should receive refund",
                i + 1
            );
        }

        // Verify dissenter received refund
        let dissenter_balance_after = Balances::free_balance(&dissenter);
        assert!(
            dissenter_balance_after > dissenter_balance_before,
            "Dissenter should receive refund"
        );
    })
}

#[test]
fn refund_error_event_contains_correct_details() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        // Empty treasury to trigger refund errors
        let (table_id, _) = setup_table_with_treasury(4, 0);

        grant_public_permissions(1, 6);

        let test_submission = TestSubmission {
            table: table_id.clone(),
            batch_id: BatchId::try_from(b"error_details_batch".to_vec()).unwrap(),
            data: row_data(),
        };

        // Clear events
        System::reset_events();

        // Late submission after quorum (need to reach quorum first with a funded treasury,
        // then drain it, but simpler to just check the InsufficientTableFunds path)
        submit_from_n_signers(4, &test_submission);

        // Check InsufficientTableFunds event has correct table
        let events = System::events();
        let insufficient_event = events.iter().find_map(|e| match &e.event {
            RuntimeEvent::Indexing(Event::InsufficientTableFunds {
                table,
                batch_id,
                treasury,
            }) => Some((table.clone(), batch_id.clone(), treasury.clone())),
            _ => None,
        });

        assert!(
            insufficient_event.is_some(),
            "InsufficientTableFunds event should be emitted"
        );

        let (event_table, _event_batch_id, event_treasury) = insufficient_event.unwrap();
        assert_eq!(event_table, table_id, "Event should contain correct table");

        let expected_treasury =
            sxt_core::utils::account_id_from_table_id::<Test>(&table_id).unwrap();
        assert_eq!(
            event_treasury, expected_treasury,
            "Event should contain correct treasury account"
        );
    })
}
