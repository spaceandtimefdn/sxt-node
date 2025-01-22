use frame_support::{assert_noop, assert_ok};
use sxt_core::permissions::{PermissionLevel, PermissionList, SmartContractsPalletPermission};
use sxt_core::smartcontracts::{ContractABI, ContractAddress};
use sxt_core::tables::Source;

use crate::mock::{new_test_ext, RuntimeEvent, RuntimeOrigin, System, Test, *};
use crate::{Contracts, Error, Event};

macro_rules! set_permission {
    ($who: expr) => {
        assert_ok!(
            Permissions::set_permissions(
                RuntimeOrigin::root(),
                $who,
                PermissionList::try_from(vec![PermissionLevel::SmartContractsPallet(
                    SmartContractsPalletPermission::UpdateABI
                )])
                .unwrap()
            ),
            ()
        );
    };
}

#[test]
fn set_smartcontract_works() {
    new_test_ext().execute_with(|| {
        let source = Source::default();
        let contract_address = ContractAddress::default();
        let contract_abi = ContractABI::default();
        let who = 1;
        set_permission!(who);

        // Call the extrinsic
        assert_ok!(SmartContracts::set_smartcontract(
            RuntimeOrigin::signed(who),
            source.clone(),
            contract_address.clone(),
            0,
            contract_abi.clone(),
        ));

        // Verify storage
        assert_eq!(
            Contracts::<Test>::get((&source, &contract_address, 1)),
            contract_abi
        );
    });
}

#[test]
fn remove_smartcontract_works() {
    new_test_ext().execute_with(|| {
        let source = Source::default();
        let contract_address = ContractAddress::default();
        let contract_abi = ContractABI::default();
        let who = 1;
        set_permission!(who);

        // Insert first
        Contracts::<Test>::insert((&source, &contract_address, 1), contract_abi.clone());

        // Ensure it exists
        assert_eq!(
            Contracts::<Test>::get((&source, &contract_address, 1)),
            contract_abi
        );

        // Call the extrinsic
        assert_ok!(SmartContracts::remove_smartcontract(
            RuntimeOrigin::signed(who),
            source.clone(),
            contract_address.clone(),
            1,
        ));

        // Verify storage removal
        assert_eq!(
            Contracts::<Test>::get((&source, &contract_address, 1)),
            ContractABI::default()
        );
    });
}

#[test]
fn remove_nonexistent_smartcontract_does_not_fail() {
    new_test_ext().execute_with(|| {
        let source = Source::default();
        let contract_address = ContractAddress::default();

        let who = 1;
        set_permission!(who);

        // Ensure it doesn't exist
        assert_eq!(
            Contracts::<Test>::get((&source, &contract_address, 1)),
            ContractABI::default()
        );

        // Call the extrinsic
        assert_ok!(SmartContracts::remove_smartcontract(
            RuntimeOrigin::signed(1),
            source.clone(),
            contract_address.clone(),
            1,
        ));

        // Storage should still be None
        assert_eq!(
            Contracts::<Test>::get((&source, &contract_address, 1)),
            ContractABI::default()
        );
    });
}
