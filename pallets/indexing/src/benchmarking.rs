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
    use pallet_tables::{CommitmentCreationCmd, UpdateTable};
    use proof_of_sql_commitment_map::CommitmentSchemeFlags;
    use sxt_core::permissions::{IndexingPalletPermission, PermissionLevel, PermissionList};
    use sxt_core::tables::{
        InsertQuorumSize,
        Source,
        TableIdentifier,
        TableName,
        TableNamespace,
        TableType,
    };

    use super::*;

    fn benchmark_integers_table_and_data() -> (UpdateTable, BatchId, RowData) {
        let ident = TableIdentifier {
            namespace: TableNamespace::try_from(b"BENCHMARK".to_vec()).unwrap(),
            name: TableName::try_from(b"INTEGERS".to_vec()).unwrap(),
        };

        let create_statement_columns = (0..64)
            .map(|col_num| alloc::format!("COL_{col_num} BIGINT NOT NULL"))
            .collect::<alloc::vec::Vec<_>>()
            .join(", ");

        let create_statement =
            alloc::format!("CREATE TABLE BENCHMARK.INTEGERS ({create_statement_columns})")
                .as_bytes()
                .to_vec()
                .try_into()
                .unwrap();

        let table_type = TableType::Testing(InsertQuorumSize {
            public: Some(3),
            privileged: None,
        });

        let commitment = CommitmentCreationCmd::Empty(CommitmentSchemeFlags::all());

        let source = Source::UserCreated(b"benchmark".to_vec().try_into().unwrap());

        let update_table = UpdateTable {
            ident,
            create_statement,
            table_type,
            commitment,
            source,
        };

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
            update_table.ident,
            batch_id.clone(),
            row_data,
        );
        assert!(Indexing::<T, I>::final_data(batch_id).is_none());
    }

    #[benchmark]
    fn submit_data_quorum_reached() {
        let (update_table, batch_id, row_data) = benchmark_integers_table_and_data();

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

        assert!(Indexing::<T, I>::final_data(batch_id.clone()).is_none());

        let caller: T::AccountId = account("dave", 0, 0);
        pallet_permissions::Permissions::<T>::insert(&caller, &permissions);
        #[extrinsic_call]
        submit_data(
            RawOrigin::Signed(caller),
            update_table.ident,
            batch_id.clone(),
            row_data,
        );
        assert!(Indexing::<T, I>::final_data(batch_id).is_some());
    }

    impl_benchmark_test_suite!(
        PalletWithApi,
        crate::mock::new_test_ext(),
        crate::mock::Test
    );
}
