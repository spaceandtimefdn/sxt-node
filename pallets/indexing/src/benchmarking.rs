//! Benchmarking setup for pallet-indexing
use alloc::vec;

use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use sp_core::crypto::Ss58Codec;

use super::*;
#[cfg(test)]
use crate::native_pallet::Pallet as PalletWithApi;
#[allow(unused)]
use crate::Pallet as Indexing;

#[allow(clippy::multiple_bound_locations)]
#[instance_benchmarks(
    where
        <T as frame_system::Config>::AccountId: Ss58Codec,
        I: NativeApi,
)]
mod benchmarks {
    use native_api::NativeApi;
    use pallet_tables::benchmarking::{integers_table_definition, schema_bytes_and_ddl_and_source};
    use pallet_tables::{CommitmentCreationCmd, UpdateTable};
    use proof_of_sql_commitment_map::CommitmentSchemeFlags;
    use sxt_core::permissions::{IndexingPalletPermission, PermissionLevel, PermissionList};
    use sxt_core::tables::{
        InsertQuorumSize, Source, TableIdentifier, TableName, TableNamespace, TableType,
    };

    use super::*;

    fn benchmark_integers_table_and_data() -> (UpdateTable, BatchId, RowData) {
        let ident = TableIdentifier {
            namespace: TableNamespace::try_from(b"BENCHMARK".to_vec()).unwrap(),
            name: TableName::try_from(b"INTEGERS".to_vec()).unwrap(),
        };

        let table_type = TableType::Testing(InsertQuorumSize {
            public: Some(3),
            privileged: None,
        });

        let update_table = integers_table_definition(ident, table_type);

        let batch_id = BatchId::try_from(b"benchmark".to_vec()).unwrap();

        let row_data_bytes = if cfg!(test) {
            include_bytes!("../benchmark-integers-row-data-small").to_vec()
        } else {
            include_bytes!("../benchmark-integers-row-data-large").to_vec()
        };

        let row_data = RowData::try_from(row_data_bytes).unwrap();

        (update_table, batch_id, row_data)
    }

    #[benchmark]
    fn submit_data_quorum_not_reached() {
        let (update_table, batch_id, row_data) = benchmark_integers_table_and_data();
        let (namespace, namespace_ddl, source) = schema_bytes_and_ddl_and_source("BENCHMARK");

        pallet_tables::Pallet::<T>::create_namespace(
            RawOrigin::<T::AccountId>::Root.into(),
            namespace,
            0,
            namespace_ddl,
            TableType::CoreBlockchain,
            source,
        )
        .expect("creating namespace in benchmark setup should work");

        let permissions = PermissionList::try_from(vec![PermissionLevel::IndexingPallet(
            IndexingPalletPermission::SubmitDataForPublicQuorum,
        )])
        .unwrap();

        pallet_tables::Pallet::<T>::create_tables(
            RawOrigin::<T::AccountId>::Root.into(),
            vec![update_table.clone()].try_into().unwrap(),
        )
        .unwrap();

        let caller: T::AccountId = account("alice", 0, 0);
        pallet_permissions::Permissions::<T>::insert(&caller, &permissions);

        #[extrinsic_call]
        submit_data(
            RawOrigin::Signed(caller),
            update_table.ident.clone(),
            batch_id.clone(),
            row_data,
        );
        let internal_batch_id = build_inner_batch_id::<T, I>(&batch_id, &update_table.ident);
        assert!(Indexing::<T, I>::final_data(internal_batch_id).is_none());
    }

    #[benchmark]
    fn submit_data_quorum_reached() {
        let (update_table, batch_id, row_data) = benchmark_integers_table_and_data();
        let (namespace, namespace_ddl, source) = schema_bytes_and_ddl_and_source("BENCHMARK");

        pallet_tables::Pallet::<T>::create_namespace(
            RawOrigin::<T::AccountId>::Root.into(),
            namespace,
            0,
            namespace_ddl,
            TableType::CoreBlockchain,
            source,
        )
        .expect("creating namespace in benchmark setup should work");

        pallet_tables::Pallet::<T>::create_tables(
            RawOrigin::<T::AccountId>::Root.into(),
            vec![update_table.clone()].try_into().unwrap(),
        )
        .unwrap();

        let permissions = PermissionList::try_from(vec![PermissionLevel::IndexingPallet(
            IndexingPalletPermission::SubmitDataForPublicQuorum,
        )])
        .unwrap();

        let caller: T::AccountId = account("alice", 0, 0);
        pallet_permissions::Permissions::<T>::insert(&caller, &permissions);
        Indexing::<T, I>::submit_data(
            RawOrigin::Signed(caller).into(),
            update_table.ident.clone(),
            batch_id.clone(),
            row_data.clone(),
        )
        .unwrap();

        let caller: T::AccountId = account("bob", 0, 0);
        pallet_permissions::Permissions::<T>::insert(&caller, &permissions);
        Indexing::<T, I>::submit_data(
            RawOrigin::Signed(caller).into(),
            update_table.ident.clone(),
            batch_id.clone(),
            row_data.clone(),
        )
        .unwrap();
        let caller: T::AccountId = account("carol", 0, 0);
        pallet_permissions::Permissions::<T>::insert(&caller, &permissions);
        Indexing::<T, I>::submit_data(
            RawOrigin::Signed(caller).into(),
            update_table.ident.clone(),
            batch_id.clone(),
            row_data.clone(),
        )
        .unwrap();

        let internal_batch_id = build_inner_batch_id::<T, I>(&batch_id, &update_table.ident);
        assert!(Indexing::<T, I>::final_data(internal_batch_id.clone()).is_none());

        let caller: T::AccountId = account("dave", 0, 0);
        pallet_permissions::Permissions::<T>::insert(&caller, &permissions);
        #[extrinsic_call]
        submit_data(
            RawOrigin::Signed(caller),
            update_table.ident.clone(),
            batch_id.clone(),
            row_data,
        );
        assert!(Indexing::<T, I>::final_data(internal_batch_id).is_some());
    }

    impl_benchmark_test_suite!(
        PalletWithApi,
        crate::mock::new_test_ext(),
        crate::mock::Test
    );
}
