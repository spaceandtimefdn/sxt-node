#![cfg(all(test, not(feature = "runtime-benchmarks")))]

use polkadot_sdk::frame_support::traits::{Get, OnRuntimeUpgrade};
use polkadot_sdk::pallet_migrations;
use polkadot_sdk::pallet_migrations::WeightInfo as _;
use proof_of_sql_commitment_map::proptest::commitment_scheme_flags;
use proof_of_sql_commitment_map::{
    CommitmentMap,
    CommitmentScheme,
    CommitmentSchemeFlags,
    CommitmentStorageMapHandler,
    TableCommitmentBytes,
    TableCommitmentBytesPerCommitmentScheme,
};
use proptest::prelude::*;
use sxt_core::proptest::table_identifier;

use crate::migrations::delete_dynamic_dory::weights::WeightInfo as _;
use crate::migrations::delete_dynamic_dory::{weights, MAX_DELETIONS_PER_BLOCK};
use crate::mock::{
    new_test_ext,
    run_to_block,
    AllPalletsWithSystem,
    MigratorServiceWeight,
    System,
    Test as T,
};
use crate::CommitmentStorageMap;

/// Strategy for generating [`TableCommitmentBytesPerCommitmentScheme`] with random bytes.
fn dummy_table_commitment_bytes_per_commitment_scheme(
    commitment_scheme_flags: impl Strategy<Value = CommitmentSchemeFlags>,
) -> impl Strategy<Value = TableCommitmentBytesPerCommitmentScheme> {
    commitment_scheme_flags
        .prop_flat_map(|commitment_scheme_flags| {
            commitment_scheme_flags
                .into_iter()
                .map(|commitment_scheme| {
                    (
                        Just(commitment_scheme),
                        proptest::collection::vec(any::<u8>(), 0..=1024),
                    )
                })
                .collect::<Vec<_>>()
        })
        .prop_map(|bytes_by_commitment_scheme_vec| {
            bytes_by_commitment_scheme_vec
                .into_iter()
                .map(|(scheme, bytes)| {
                    scheme.into_any_concrete(TableCommitmentBytes {
                        data: bytes.try_into().unwrap(),
                    })
                })
                .collect()
        })
}

proptest! {
    #[test]
    fn lazy_migration_works(
        tables in proptest::collection::vec(
            (
                table_identifier(),
                dummy_table_commitment_bytes_per_commitment_scheme(commitment_scheme_flags())
            ),
            1..=64
        )
    )
    {
        new_test_ext().execute_with(|| {
            polkadot_sdk::frame_support::__private::sp_tracing::try_init_simple();
            let mut handler = CommitmentStorageMapHandler::<CommitmentStorageMap<T>>::new();


            tables.iter().for_each(|(table_identifier, commitments)| {
                handler
                    .create_commitments(table_identifier.clone(), commitments.clone())
                    .unwrap();
            });

            fn count_commitments_by_scheme() -> (usize, usize) {
                CommitmentStorageMap::<T>::iter().fold(
                    (0, 0),
                    |(hyperkzg_count, dynamic_dory_count), (_, commitment_scheme, _)| {
                        match commitment_scheme {
                            CommitmentScheme::HyperKzg => (hyperkzg_count + 1, dynamic_dory_count),
                            CommitmentScheme::DynamicDory => (hyperkzg_count, dynamic_dory_count + 1),
                        }
                    },
                )
            }

            let (initial_hyperkzg_count, initial_dynamic_dory_count) = count_commitments_by_scheme();

            dbg!(&initial_hyperkzg_count, &initial_dynamic_dory_count);

            // Give it enough weight do do exactly 16 iterations:
            let limit = <T as pallet_migrations::Config>::WeightInfo::progress_mbms_none()
                + pallet_migrations::Pallet::<T>::exec_migration_max_weight()
                + weights::SubstrateWeight::<T>::step() * 16;
            MigratorServiceWeight::set(&limit);

            System::set_block_number(1);
            AllPalletsWithSystem::on_runtime_upgrade(); // onboard MBMs
            let num_commitments = tables.iter().flat_map(|(_, commitments)| commitments.clone()).count();

            let mut previous_dynamic_dory_count = initial_dynamic_dory_count;
            for block in 2..=(num_commitments.div_ceil(16usize.div_ceil(MAX_DELETIONS_PER_BLOCK) + 1) + 2) {
                run_to_block(block as u64);

                let (hyperkzg_count, dynamic_dory_count) = count_commitments_by_scheme();

                dbg!(&block, &hyperkzg_count, &dynamic_dory_count);

                assert!(dynamic_dory_count <= previous_dynamic_dory_count );
                assert_eq!(hyperkzg_count, initial_hyperkzg_count);

                previous_dynamic_dory_count = dynamic_dory_count;
            }

            let (hyperkzg_count, dynamic_dory_count) = count_commitments_by_scheme();
            assert_eq!(dynamic_dory_count, 0);
            assert_eq!(hyperkzg_count, initial_hyperkzg_count);
        });
    }
}
