//! Benchmarking setup for pallet-template
#![cfg(feature = "runtime-benchmarks")]
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use sp_core::crypto::Ss58Codec;
use sp_core::{H160, U256};
use sxt_core::system_contracts::ContractInfo;

use super::*;
#[allow(unused)]
use crate::Pallet as SystemContracts;

fn sample_contract_info() -> ContractInfo {
    ContractInfo {
        chain_id: U256::from(123u32),
        address: H160::zero(),
    }
}

#[benchmarks(where <T as frame_system::Config>::AccountId: Ss58Codec)]
mod benchmarks {

    use super::*;

    #[benchmark]
    fn set_staking_contract() {
        let contract_info = sample_contract_info();

        // Set staking contract.
        #[extrinsic_call]
        SystemContracts::set_staking_contract(RawOrigin::Root, contract_info);

        assert_eq!(SystemContracts::<T>::staking_contract(), contract_info);
    }

    #[benchmark]
    fn set_messaging_contract() {
        let contract_info = sample_contract_info();

        // Set messaging contract.
        #[extrinsic_call]
        SystemContracts::set_messaging_contract(RawOrigin::Root, contract_info);

        assert_eq!(SystemContracts::<T>::messaging_contract(), contract_info);
    }

    impl_benchmark_test_suite!(
        SystemContracts,
        crate::mock::new_test_ext(),
        crate::mock::Test
    );
}
