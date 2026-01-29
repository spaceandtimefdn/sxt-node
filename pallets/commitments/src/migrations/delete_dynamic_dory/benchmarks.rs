//! Benchmark the multi-block-migration.

#![cfg(feature = "runtime-benchmarks")]

use polkadot_sdk::frame_benchmarking::v2::*;
use polkadot_sdk::frame_support::migrations::SteppedMigration;
use polkadot_sdk::frame_support::traits::Get;
use polkadot_sdk::frame_support::weights::WeightMeter;
use proof_of_sql_commitment_map::{
    CommitmentScheme,
    TableCommitmentBytes,
    TableCommitmentMaxLength,
};
use sxt_core::tables::{TableIdentifier, TableName, TableNamespace};

use crate::migrations::delete_dynamic_dory::weights::WeightInfo;
use crate::migrations::delete_dynamic_dory::{weights, DeleteDynamicDoryCommitmentsLazyMigration};
use crate::{CommitmentStorageMap, Config, Pallet};

#[benchmarks]
mod benches {
    use super::*;

    /// Benchmark a single step of the `DeleteDynamicDoryCommitmentsLazyMigration` migration.
    #[benchmark]
    fn step() {
        let table_identifier = TableIdentifier {
            namespace: TableNamespace::try_from(b"BENCHMARK".to_vec()).unwrap(),
            name: TableName::try_from(b"DELETE_DYNAMIC_DORY".to_vec()).unwrap(),
        };
        let dummy_commitment_data = TableCommitmentBytes {
            data: core::iter::repeat_n(1, <TableCommitmentMaxLength as Get<u32>>::get() as usize)
                .collect::<alloc::vec::Vec<_>>()
                .try_into()
                .unwrap(),
        };

        CommitmentStorageMap::<T>::insert(
            table_identifier.clone(),
            CommitmentScheme::DynamicDory,
            dummy_commitment_data,
        );
        let mut meter = WeightMeter::new();

        #[block]
        {
            DeleteDynamicDoryCommitmentsLazyMigration::<T, weights::SubstrateWeight<T>>::step(
                None, &mut meter,
            )
            .unwrap();
        }

        assert_eq!(
            crate::CommitmentStorageMap::<T>::get(table_identifier, CommitmentScheme::DynamicDory),
            None
        );
        // uses twice the weight once for migration and then for checking if there is another key.
        assert_eq!(meter.consumed(), weights::SubstrateWeight::<T>::step() * 2);
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
