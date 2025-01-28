//! Benchmarking setup for pallet-indexing
use alloc::vec;

use frame_benchmarking::v2::*;
use frame_system::RawOrigin;

use super::*;
#[cfg(test)]
use crate::native_pallet::Pallet as PalletWithApi;
#[allow(unused)]
use crate::Pallet as Indexing;

#[allow(clippy::multiple_bound_locations)]
#[instance_benchmarks(where I: NativeApi)]
mod benchmarks {
    use native_api::NativeApi;
    use sxt_core::permissions::{IndexingPalletPermission, PermissionLevel, PermissionList};
    use sxt_core::tables::{TableIdentifier, TableName, TableNamespace};

    use super::*;

    #[benchmark]
    fn submit_data_quorum_not_reached() {
        let table_id = TableIdentifier {
            namespace: TableNamespace::try_from(b"ETHEREUM".to_vec()).unwrap(),
            name: TableName::try_from(b"TRANSACTIONS".to_vec()).unwrap(),
        };

        let test_batch_id = BatchId::try_from(b"test_batch".to_vec()).unwrap();

        let test_submission =
            RowData::try_from(include_bytes!("../benchmark-ethereum-transactions-batch").to_vec())
                .unwrap();

        let permissions = PermissionList::try_from(vec![PermissionLevel::IndexingPallet(
            IndexingPalletPermission::SubmitDataForPublicQuorum,
        )])
        .unwrap();

        let caller: T::AccountId = account("alice", 0, 0);
        pallet_permissions::Permissions::<T>::insert(&caller, &permissions);

        #[extrinsic_call]
        submit_data(
            RawOrigin::Signed(caller),
            table_id,
            test_batch_id.clone(),
            test_submission,
        );
        assert!(Indexing::<T, I>::final_data(test_batch_id.clone()).is_none());
    }

    #[benchmark]
    fn submit_data_quorum_reached() {
        let table_id = TableIdentifier {
            namespace: TableNamespace::try_from(b"ETHEREUM".to_vec()).unwrap(),
            name: TableName::try_from(b"TRANSACTIONS".to_vec()).unwrap(),
        };

        let test_batch_id = BatchId::try_from(b"test_batch".to_vec()).unwrap();

        let test_submission =
            RowData::try_from(include_bytes!("../benchmark-ethereum-transactions-batch").to_vec())
                .unwrap();

        let permissions = PermissionList::try_from(vec![PermissionLevel::IndexingPallet(
            IndexingPalletPermission::SubmitDataForPublicQuorum,
        )])
        .unwrap();

        let caller: T::AccountId = account("alice", 0, 0);
        pallet_permissions::Permissions::<T>::insert(&caller, &permissions);
        Indexing::<T, I>::submit_data(
            RawOrigin::Signed(caller).into(),
            table_id.clone(),
            test_batch_id.clone(),
            test_submission.clone(),
        )
        .unwrap();
        let caller: T::AccountId = account("bob", 0, 0);
        pallet_permissions::Permissions::<T>::insert(&caller, &permissions);
        Indexing::<T, I>::submit_data(
            RawOrigin::Signed(caller).into(),
            table_id.clone(),
            test_batch_id.clone(),
            test_submission.clone(),
        )
        .unwrap();
        let caller: T::AccountId = account("carol", 0, 0);
        pallet_permissions::Permissions::<T>::insert(&caller, &permissions);
        Indexing::<T, I>::submit_data(
            RawOrigin::Signed(caller).into(),
            table_id.clone(),
            test_batch_id.clone(),
            test_submission.clone(),
        )
        .unwrap();

        assert!(Indexing::<T, I>::final_data(test_batch_id.clone()).is_none());

        let caller: T::AccountId = account("dave", 0, 0);
        pallet_permissions::Permissions::<T>::insert(&caller, &permissions);
        #[extrinsic_call]
        submit_data(
            RawOrigin::Signed(caller),
            table_id,
            test_batch_id.clone(),
            test_submission,
        );
        assert!(Indexing::<T, I>::final_data(test_batch_id).is_some());
    }

    impl_benchmark_test_suite!(
        PalletWithApi,
        crate::mock::new_test_ext(),
        crate::mock::Test
    );
}
