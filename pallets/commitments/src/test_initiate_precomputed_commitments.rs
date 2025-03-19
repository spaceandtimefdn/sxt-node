use frame_support::assert_noop;
use proof_of_sql::base::commitment::TableCommitment;
use proof_of_sql::proof_primitive::dory::DynamicDoryCommitment;
use proof_of_sql_commitment_map::{
    CommitmentScheme,
    KeyExistsError,
    TableCommitmentBytes,
    TableCommitmentBytesPerCommitmentScheme,
};
use sxt_core::tables::{TableIdentifier, TableName, TableNamespace};

use crate::mock::*;

#[test]
#[allow(deprecated)]
fn we_can_initiate_precomputed_commitments() {
    new_test_ext().execute_with(|| {
        let table_id = TableIdentifier {
            namespace: TableNamespace::try_from(b"test".to_owned().to_vec()).unwrap(),
            name: TableName::try_from(b"table".to_owned().to_vec()).unwrap(),
        };

        let commitment =
            TableCommitmentBytes::try_from(&TableCommitment::<DynamicDoryCommitment>::default())
                .unwrap();

        let per_commitment_scheme = TableCommitmentBytesPerCommitmentScheme {
            hyper_kzg: None,
            dynamic_dory: Some(commitment.clone()),
        };

        CommitmentsModule::initiate_precomputed_commitments(
            table_id.clone(),
            per_commitment_scheme,
        )
        .unwrap();

        assert_eq!(
            CommitmentsModule::table_commitment(&table_id, CommitmentScheme::HyperKzg),
            None
        );

        assert_eq!(
            CommitmentsModule::table_commitment(&table_id, CommitmentScheme::DynamicDory),
            Some(commitment)
        );
    });
}

#[test]
#[allow(deprecated)]
fn we_cannot_initiate_commitments_if_table_already_exists() {
    new_test_ext().execute_with(|| {
        let table_id = TableIdentifier {
            namespace: TableNamespace::try_from(b"test".to_owned().to_vec()).unwrap(),
            name: TableName::try_from(b"table".to_owned().to_vec()).unwrap(),
        };

        let commitment =
            TableCommitmentBytes::try_from(&TableCommitment::<DynamicDoryCommitment>::default())
                .unwrap();

        let per_commitment_scheme = TableCommitmentBytesPerCommitmentScheme {
            hyper_kzg: None,
            dynamic_dory: Some(commitment.clone()),
        };

        CommitmentsModule::initiate_precomputed_commitments(
            table_id.clone(),
            per_commitment_scheme.clone(),
        )
        .unwrap();

        assert_noop!(
            CommitmentsModule::initiate_precomputed_commitments(
                table_id.clone(),
                per_commitment_scheme
            ),
            KeyExistsError { key: table_id },
        );
    });
}
