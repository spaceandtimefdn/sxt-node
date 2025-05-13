use alloc::vec;

use proof_of_sql_commitment_map::{AnyCommitmentScheme, CommitmentScheme, TableCommitmentBytes};
use sxt_core::tables::TableIdentifier;

use crate::mock::{new_test_ext, CommitmentsModule, Test};

fn fake_commitment(seed: u8) -> TableCommitmentBytes {
    TableCommitmentBytes {
        data: vec![seed; seed as usize].try_into().unwrap(),
    }
}

#[test]
fn we_can_get_table_commitments_of_any_scheme() {
    new_test_ext().execute_with(|| {
        let table_id_0 = TableIdentifier {
            namespace: b"ANIMAL".to_vec().try_into().unwrap(),
            name: b"POPULATION".to_vec().try_into().unwrap(),
        };
        let table_id_1 = TableIdentifier {
            namespace: b"LUMBER".to_vec().try_into().unwrap(),
            name: b"YARDS".to_vec().try_into().unwrap(),
        };

        let tables = [table_id_0.clone(), table_id_1.clone()];

        // no commitments for any scheme
        assert_eq!(
            CommitmentsModule::table_commitments_any_scheme(&tables),
            None
        );

        // no commitments for one scheme, incomplete commitments for the other
        crate::CommitmentStorageMap::<Test>::insert(
            &table_id_0,
            CommitmentScheme::DynamicDory,
            fake_commitment(0),
        );
        assert_eq!(
            CommitmentsModule::table_commitments_any_scheme(&tables),
            None
        );

        // incomplete commitments for both schemes
        crate::CommitmentStorageMap::<Test>::insert(
            &table_id_1,
            CommitmentScheme::HyperKzg,
            fake_commitment(1),
        );
        assert_eq!(
            CommitmentsModule::table_commitments_any_scheme(&tables),
            None
        );

        // incomplete commitments for one scheme, complete commitments for the other
        crate::CommitmentStorageMap::<Test>::insert(
            &table_id_1,
            CommitmentScheme::DynamicDory,
            fake_commitment(2),
        );
        assert_eq!(
            CommitmentsModule::table_commitments_any_scheme(&tables),
            Some(AnyCommitmentScheme::DynamicDory(vec![
                fake_commitment(0),
                fake_commitment(2)
            ])),
        );

        // complete commitments for both (variant order determines priority)
        crate::CommitmentStorageMap::<Test>::insert(
            &table_id_0,
            CommitmentScheme::HyperKzg,
            fake_commitment(3),
        );
        assert_eq!(
            CommitmentsModule::table_commitments_any_scheme(&tables),
            Some(AnyCommitmentScheme::HyperKzg(vec![
                fake_commitment(3),
                fake_commitment(1)
            ])),
        );
    })
}
