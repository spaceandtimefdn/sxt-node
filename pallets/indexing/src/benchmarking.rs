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
    use pallet_tables::benchmarking::{integers_table_definition, schema_bytes_and_ddl_and_source};
    use pallet_tables::pallet::BlockEnforcementMode;
    use pallet_tables::{BlockEnforcement, CommitmentCreationCmd, UpdateTable};
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

    fn benchmark_integers_table_and_data(
        commitment_schemes: CommitmentSchemeFlags,
    ) -> (UpdateTable, BatchId, RowData) {
        let ident = TableIdentifier {
            namespace: TableNamespace::try_from(b"BENCHMARK".to_vec()).unwrap(),
            name: TableName::try_from(b"INTEGERS".to_vec()).unwrap(),
        };

        let table_type = TableType::Testing(InsertQuorumSize {
            public: Some(3),
            privileged: None,
        });

        let update_table = integers_table_definition(ident, table_type, commitment_schemes);

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
        let (update_table, batch_id, row_data) =
            benchmark_integers_table_and_data(CommitmentSchemeFlags::all());
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
        commitment_schemes: CommitmentSchemeFlags,
    ) -> (T::AccountId, TableIdentifier, BatchId, RowData)
    where
        T: Config<I>,
        <T as frame_system::Config>::AccountId: Ss58Codec,
        I: NativeApi,
    {
        let (update_table, batch_id, row_data) =
            benchmark_integers_table_and_data(commitment_schemes);
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
    fn submit_data_quorum_reached_dynamic_dory() {
        let (caller, table_identifier, batch_id, row_data) =
            setup_quorum_reached_benchmark::<T, I>(CommitmentSchemeFlags {
                dynamic_dory: true,
                ..Default::default()
            });

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
    fn submit_data_quorum_reached_hyper_kzg() {
        let (caller, table_identifier, batch_id, row_data) =
            setup_quorum_reached_benchmark::<T, I>(CommitmentSchemeFlags {
                hyper_kzg: true,
                ..Default::default()
            });

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
    fn set_block_number() {
        let table = TableIdentifier {
            namespace: TableNamespace::try_from(b"BENCHMARK".to_vec()).unwrap(),
            name: TableName::try_from(b"INTEGERS".to_vec()).unwrap(),
        };
        let block_number: u64 = 42;

        #[extrinsic_call]
        set_block_number(RawOrigin::Root, table.clone(), block_number);

        assert_eq!(Indexing::<T, I>::block_numbers(&table), Some(block_number));
    }

    #[benchmark]
    fn submit_empty_blocks() {
        let (update_table, _, _) = benchmark_integers_table_and_data(CommitmentSchemeFlags::all());
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

        let batch_id = BatchId::try_from(b"benchmark_empty".to_vec()).unwrap();
        let start_block_number: u64 = 100;
        let end_block_number: u64 = 105;

        // Pre-submit from alice, bob, carol so the measured call (dave) is the one that
        // reaches quorum and exercises the full finalization path (writes FinalData /
        // BlockNumbers and runs the block-enforcement check).
        for name in ["alice", "bob", "carol"] {
            let submitter: T::AccountId = account(name, 0, 0);
            pallet_permissions::Permissions::<T>::insert(&submitter, &permissions);
            Indexing::<T, I>::submit_empty_blocks(
                RawOrigin::Signed(submitter).into(),
                update_table.ident.clone(),
                batch_id.clone(),
                start_block_number,
                end_block_number,
            )
            .expect("pre-submission in benchmark setup should succeed");
        }

        // Enable increasing block-number enforcement and seed the previous block number so
        // the enforcement-check branch is exercised during finalization (worst-case path).
        BlockEnforcement::<T>::insert(&update_table.ident, BlockEnforcementMode::Increasing);
        BlockNumbers::<T, I>::insert(&update_table.ident, start_block_number - 1);

        let caller: T::AccountId = account("dave", 0, 0);
        pallet_permissions::Permissions::<T>::insert(&caller, &permissions);

        #[extrinsic_call]
        submit_empty_blocks(
            RawOrigin::Signed(caller),
            update_table.ident.clone(),
            batch_id,
            start_block_number,
            end_block_number,
        );

        assert_eq!(
            BlockNumbers::<T, I>::get(&update_table.ident),
            Some(end_block_number)
        );
    }

    impl_benchmark_test_suite!(
        PalletWithApi,
        crate::mock::new_test_ext(),
        crate::mock::Test
    );
}
