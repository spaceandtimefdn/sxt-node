//! Benchmarking setup for pallet-indexing
use alloc::vec;

use polkadot_sdk::frame_benchmarking::v2::*;
use polkadot_sdk::frame_system;
use polkadot_sdk::frame_system::RawOrigin;
use polkadot_sdk::sp_core::crypto::Ss58Codec;

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
    use on_chain_table::{OnChainColumn, OnChainTable};
    use pallet_tables::benchmarking::schema_bytes_and_ddl_and_source;
    use pallet_tables::{CommitmentCreationCmd, UpdateTable};
    use proof_of_sql_commitment_map::CommitmentSchemeFlags;
    use sqlparser::ast::Ident;
    use sxt_core::permissions::{IndexingPalletPermission, PermissionLevel, PermissionList};
    use sxt_core::tables::{
        InsertQuorumSize,
        Source,
        TableIdentifier,
        TableName,
        TableNamespace,
        TableType,
        MAX_COLS_PER_TABLE,
    };

    use super::*;

    fn expensive_on_chain_table(num_rows: usize, num_cols: usize) -> OnChainTable {
        OnChainTable::try_from_iter((0..num_cols).map(|col_num| {
            let column = OnChainColumn::VarBinary(
                (0..num_rows)
                    .map(|row_num| {
                        let element_num = (row_num * num_cols) + col_num;

                        element_num
                            .to_le_bytes()
                            .into_iter()
                            .chain(core::iter::repeat(0))
                            .take(256)
                            .collect()
                    })
                    .collect(),
            );

            let name = Ident::new(alloc::format!("COL_{col_num}"));
            (name, column)
        }))
        .unwrap()
    }

    fn expensive_update_table(
        commitment_schemes: CommitmentSchemeFlags,
        num_cols: usize,
    ) -> UpdateTable {
        let create_statement_columns = (0..num_cols)
            .map(|col_num| alloc::format!("COL_{col_num} BINARY NOT NULL"))
            .collect::<alloc::vec::Vec<_>>()
            .join(", ");

        let ident = TableIdentifier {
            namespace: TableNamespace::try_from(b"BENCHMARK".to_vec()).unwrap(),
            name: TableName::try_from(b"EXPENSIVE_BINARY".to_vec()).unwrap(),
        };

        let create_statement_table_identifier = alloc::format!(
            "{}.{}",
            core::str::from_utf8(ident.namespace.as_slice()).unwrap(),
            core::str::from_utf8(ident.name.as_slice()).unwrap()
        );

        let create_statement = alloc::format!(
            "CREATE TABLE {create_statement_table_identifier} ({create_statement_columns})"
        )
        .as_bytes()
        .to_vec()
        .try_into()
        .unwrap();

        let table_type = TableType::Testing(InsertQuorumSize {
            public: Some(3),
            privileged: None,
        });

        let commitment = CommitmentCreationCmd::Empty(commitment_schemes);

        let source = Source::UserCreated(b"benchmark".to_vec().try_into().unwrap());

        UpdateTable {
            ident,
            create_statement,
            table_type,
            commitment,
            source,
        }
    }

    fn benchmark_expensive_table_and_data<I: NativeApi>(
        num_rows: usize,
        num_cols: usize,
        commitment_schemes: CommitmentSchemeFlags,
    ) -> (UpdateTable, BatchId, RowData) {
        let update_table = expensive_update_table(commitment_schemes, num_cols);

        let batch_id = BatchId::try_from(b"benchmark".to_vec()).unwrap();

        let table = if cfg!(test) {
            expensive_on_chain_table(4, num_cols)
        } else {
            expensive_on_chain_table(num_rows, num_cols)
        };

        let row_data_bytes = I::on_chain_table_to_record_batch(table.try_into().unwrap()).unwrap();

        let row_data = row_data_bytes.row_data;

        (update_table, batch_id, row_data)
    }

    #[benchmark]
    fn submit_data_quorum_not_reached(r: Linear<0, 8>) {
        let (update_table, batch_id, row_data) = benchmark_expensive_table_and_data::<I>(
            r as usize,
            MAX_COLS_PER_TABLE as usize,
            CommitmentSchemeFlags::all(),
        );
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

    fn setup_quorum_reached_benchmark<T, I>(
        num_rows: usize,
        num_cols: usize,
        commitment_schemes: CommitmentSchemeFlags,
    ) -> (T::AccountId, TableIdentifier, BatchId, RowData)
    where
        T: Config<I>,
        <T as frame_system::Config>::AccountId: Ss58Codec,
        I: NativeApi,
    {
        let (update_table, batch_id, row_data) =
            benchmark_expensive_table_and_data::<I>(num_rows, num_cols, commitment_schemes);
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

        let caller: T::AccountId = account("dave", 0, 0);
        pallet_permissions::Permissions::<T>::insert(&caller, &permissions);

        (caller, update_table.ident, batch_id, row_data)
    }

    #[benchmark]
    fn submit_data_quorum_reached_dynamic_dory(r: Linear<0, 8>) {
        let (caller, table_identifier, batch_id, row_data) = setup_quorum_reached_benchmark::<T, I>(
            r as usize,
            MAX_COLS_PER_TABLE as usize,
            CommitmentSchemeFlags {
                dynamic_dory: true,
                ..Default::default()
            },
        );

        let internal_batch_id = build_inner_batch_id::<T, I>(&batch_id, &table_identifier);
        assert!(Indexing::<T, I>::final_data(internal_batch_id.clone()).is_none());

        #[extrinsic_call]
        submit_data(
            RawOrigin::Signed(caller),
            table_identifier.clone(),
            batch_id.clone(),
            row_data,
        );

        assert!(Indexing::<T, I>::final_data(internal_batch_id).is_some());
    }

    #[benchmark]
    fn submit_data_quorum_reached_hyper_kzg(r: Linear<0, 8>) {
        let (caller, table_identifier, batch_id, row_data) = setup_quorum_reached_benchmark::<T, I>(
            r as usize,
            MAX_COLS_PER_TABLE as usize,
            CommitmentSchemeFlags {
                hyper_kzg: true,
                ..Default::default()
            },
        );

        let internal_batch_id = build_inner_batch_id::<T, I>(&batch_id, &table_identifier);
        assert!(Indexing::<T, I>::final_data(internal_batch_id.clone()).is_none());

        #[extrinsic_call]
        submit_data(
            RawOrigin::Signed(caller),
            table_identifier.clone(),
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
