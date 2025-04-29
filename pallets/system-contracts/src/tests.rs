use frame_support::{assert_noop, assert_ok};
use sp_core::{H160, U256};
use sxt_core::system_contracts::ContractInfo;

use crate::mock::*;
use crate::Event;

fn sample_contract_info() -> ContractInfo {
    ContractInfo {
        chain_id: U256::from(123u32),
        address: H160::random(),
    }
}

#[test]
fn we_can_set_contracts() {
    new_test_ext().execute_with(|| {
        // Go past genesis block so events get deposited
        System::set_block_number(1);

        let contract_info = sample_contract_info();
        // Set staking contract.
        assert_ok!(SystemContracts::set_staking_contract(
            RuntimeOrigin::root(),
            contract_info
        ));
        assert_eq!(SystemContracts::staking_contract(), contract_info);
        System::assert_last_event(Event::StakingContractUpdated { contract_info }.into());

        // Set messaging contract.
        assert_ok!(SystemContracts::set_messaging_contract(
            RuntimeOrigin::root(),
            contract_info
        ));
        assert_eq!(SystemContracts::messaging_contract(), contract_info);
        System::assert_last_event(Event::MessagingContractUpdated { contract_info }.into());
    });
}

#[test]
fn non_root_cannot_set_contracts() {
    new_test_ext().execute_with(|| {
        // Go past genesis block so events get deposited
        System::set_block_number(1);

        let contract_info = sample_contract_info();

        assert_noop!(
            SystemContracts::set_staking_contract(RuntimeOrigin::signed(1), contract_info,),
            sp_runtime::DispatchError::BadOrigin
        );

        assert_noop!(
            SystemContracts::set_messaging_contract(RuntimeOrigin::signed(1), contract_info,),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}
