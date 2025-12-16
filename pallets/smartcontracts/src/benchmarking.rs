//! Benchmarking setup for pallet-smartcontracts
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use scale_info::prelude::vec;
use sxt_core::permissions::{PermissionLevel, SmartContractsPalletPermission};
use sxt_core::smartcontracts::{Contract, ContractAddress, ContractDetails, NormalContract};
use sxt_core::tables::Source;

use super::*;
#[cfg(test)]
use crate::native_pallet::Pallet as PalletWithApi;
#[allow(unused)]
use crate::Pallet as SmartContracts;

pub fn grant_update_abi<T: pallet_permissions::Config>(account: T::AccountId) {
    pallet_permissions::Pallet::<T>::add_proxy_permission(
        RawOrigin::Root.into(),
        account,
        PermissionLevel::SmartContractsPallet(SmartContractsPalletPermission::UpdateABI),
    )
    .unwrap();
}

fn benchmark_contract_definition() -> Contract {
    let (target_schema, ddl_statement, source) =
        pallet_tables::benchmarking::schema_bytes_and_ddl_and_source("MY_CONTRACT");
    let address: ContractAddress = vec![0; 64].try_into().unwrap();
    let contract_abi = vec![0; 256].try_into().unwrap();

    Contract::Normal(NormalContract {
        details: ContractDetails {
            source,
            address,
            abi: Some(contract_abi),
            starting_block: Some(100),
            target_schema: Some(target_schema),
            contract_name: None,
            event_details: None,
            ddl_statement: Some(ddl_statement),
        },
    })
}

#[allow(clippy::multiple_bound_locations)]
#[instance_benchmarks(
    where
        <T as frame_system::Config>::AccountId: Ss58Codec,
        I: NativeApi,
)]
mod benchmarks {
    use native_api::NativeApi;
    use pallet_tables::benchmarking::grant_edit_schema;
    use proof_of_sql_commitment_map::CommitmentSchemeFlags;
    use sp_core::crypto::Ss58Codec;
    use sxt_core::tables::{TableIdentifier, TableType};

    use super::*;

    #[benchmark]
    fn add_smartcontract_zero_tables() {
        let caller: T::AccountId = whitelisted_caller();

        grant_update_abi::<T>(caller.clone());
        grant_edit_schema::<T>(caller.clone());

        let contract_definition = benchmark_contract_definition();

        #[extrinsic_call]
        SmartContracts::add_smartcontract(
            RawOrigin::Signed(caller),
            contract_definition.clone(),
            Default::default(),
        );

        let Contract::Normal(NormalContract {
            details: ContractDetails {
                source, address, ..
            },
        }) = &contract_definition
        else {
            panic!("should be normal contract")
        };

        assert_eq!(
            ContractStorage::<T, I>::get(source, address),
            Some(contract_definition)
        );
    }

    #[benchmark]
    fn add_smartcontract_one_table() {
        let caller: T::AccountId = whitelisted_caller();

        grant_update_abi::<T>(caller.clone());
        grant_edit_schema::<T>(caller.clone());

        let contract_definition = benchmark_contract_definition();

        let table_identifier = TableIdentifier::from_str_unchecked("TABLE", "MY_CONTRACT");

        let table_definition = pallet_tables::benchmarking::integers_table_definition(
            table_identifier.clone(),
            TableType::SCI,
            CommitmentSchemeFlags::all(),
        );

        #[extrinsic_call]
        SmartContracts::add_smartcontract(
            RawOrigin::Signed(caller),
            contract_definition.clone(),
            vec![table_definition].try_into().unwrap(),
        );

        let Contract::Normal(NormalContract {
            details: ContractDetails {
                source, address, ..
            },
        }) = &contract_definition
        else {
            panic!("should be normal contract")
        };

        assert_eq!(
            ContractStorage::<T, I>::get(source, address),
            Some(contract_definition)
        );
        assert!(pallet_tables::Schemas::<T>::contains_key(
            &table_identifier.namespace,
            &table_identifier.name,
        ));
    }

    #[benchmark]
    fn remove_smartcontract_zero_tables() {
        let caller: T::AccountId = whitelisted_caller();

        grant_update_abi::<T>(caller.clone());
        grant_edit_schema::<T>(caller.clone());

        let contract_definition = benchmark_contract_definition();

        SmartContracts::<T, I>::add_smartcontract(
            RawOrigin::Signed(caller.clone()).into(),
            contract_definition.clone(),
            Default::default(),
        )
        .unwrap();

        let Contract::Normal(NormalContract {
            details: ContractDetails {
                source, address, ..
            },
        }) = contract_definition.clone()
        else {
            panic!("should be normal contract")
        };

        assert_eq!(
            ContractStorage::<T, I>::get(&source, &address),
            Some(contract_definition)
        );

        #[extrinsic_call]
        SmartContracts::remove_smartcontract(
            RawOrigin::Signed(caller),
            source.clone(),
            address.clone(),
        );

        assert_eq!(ContractStorage::<T, I>::get(&source, &address), None);
    }

    #[benchmark]
    fn remove_smartcontract_one_table() {
        let caller: T::AccountId = whitelisted_caller();

        grant_update_abi::<T>(caller.clone());
        grant_edit_schema::<T>(caller.clone());

        let contract_definition = benchmark_contract_definition();

        let table_identifier = TableIdentifier::from_str_unchecked("TABLE", "MY_CONTRACT");

        let table_definition = pallet_tables::benchmarking::integers_table_definition(
            table_identifier.clone(),
            TableType::SCI,
            CommitmentSchemeFlags::all(),
        );

        SmartContracts::<T, I>::add_smartcontract(
            RawOrigin::Signed(caller.clone()).into(),
            contract_definition.clone(),
            vec![table_definition].try_into().unwrap(),
        )
        .unwrap();

        let Contract::Normal(NormalContract {
            details: ContractDetails {
                source, address, ..
            },
        }) = contract_definition.clone()
        else {
            panic!("should be normal contract")
        };

        assert_eq!(
            ContractStorage::<T, I>::get(&source, &address),
            Some(contract_definition)
        );
        assert!(pallet_tables::Schemas::<T>::contains_key(
            &table_identifier.namespace,
            &table_identifier.name,
        ));

        #[extrinsic_call]
        SmartContracts::remove_smartcontract(
            RawOrigin::Signed(caller),
            source.clone(),
            address.clone(),
        );

        assert_eq!(ContractStorage::<T, I>::get(&source, &address), None);
        assert!(!pallet_tables::Schemas::<T>::contains_key(
            &table_identifier.namespace,
            &table_identifier.name,
        ));
    }

    impl_benchmark_test_suite!(
        PalletWithApi,
        crate::mock::new_test_ext(),
        crate::mock::Test
    );
}
