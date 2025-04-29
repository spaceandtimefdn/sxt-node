use frame_support::storage::bounded_vec::BoundedVec;
use frame_support::{assert_noop, assert_ok};
use sxt_core::permissions::{
    PermissionLevel,
    PermissionList,
    SmartContractsPalletPermission,
    TablesPalletPermission,
};
use sxt_core::smartcontracts::{
    Contract,
    ContractABI,
    ContractAddress,
    ContractDetails,
    ImplementationContract,
    NormalContract,
    ProxyContract,
};
use sxt_core::tables::Source;

use crate::mock::{new_test_ext, RuntimeOrigin, System, Test, *};
use crate::{ContractStorage, Error, Event};

/// Helper macro to set permissions for a given user
macro_rules! set_permission {
    ($who: expr) => {
        assert_ok!(
            Permissions::set_permissions(
                RuntimeOrigin::root(),
                $who,
                PermissionList::try_from(vec![
                    PermissionLevel::SmartContractsPallet(
                        SmartContractsPalletPermission::UpdateABI
                    ),
                    PermissionLevel::TablesPallet(TablesPalletPermission::EditSchema),
                ])
                .unwrap()
            ),
            ()
        );
    };
}

/// Creates a `ContractAddress` from a fixed slice of bytes.
fn create_contract_address() -> ContractAddress {
    BoundedVec::try_from(vec![0xAA; 64]).expect("Contract address should fit within 64 bytes")
}

/// Creates a `ContractABI` with dummy data.
fn create_contract_abi() -> ContractABI {
    BoundedVec::try_from(vec![0xBB; 256]).expect("Contract ABI should fit within 256 bytes")
}

/// **Test: Adding a Normal Smart Contract Works**
#[test]
fn add_smartcontract_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let source = Source::default();
        let contract_address = create_contract_address();
        let contract_abi = Some(create_contract_abi());
        let who = 1;
        set_permission!(who);

        let normal_contract = Contract::Normal(NormalContract {
            details: ContractDetails {
                source: source.clone(),
                address: contract_address.clone(),
                abi: contract_abi.clone(),
                starting_block: Some(100),
                target_schema: Some(
                    BoundedVec::try_from("test".as_bytes().to_vec()).expect("should always work"),
                ),
                contract_name: None,
                event_details: None,
                ddl_statement: Some(
                    BoundedVec::try_from("test".as_bytes().to_vec()).expect("should always work"),
                ),
            },
        });

        // Call the extrinsic
        assert_ok!(SmartContracts::add_smartcontract(
            RuntimeOrigin::signed(who),
            normal_contract.clone(),
            Default::default(),
        ));

        // Verify storage
        assert_eq!(
            ContractStorage::<Test>::get(source.clone(), contract_address.clone()),
            Some(normal_contract)
        );

        // Verify event emitted
        System::assert_has_event(
            Event::SmartContractAdded {
                owner: Some(who),
                source,
                address: contract_address,
            }
            .into(),
        );
    });
}

/// **Test: Adding a Proxy Smart Contract Works**
#[test]
fn add_proxy_smartcontract_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let source = Source::default();
        let proxy_address = create_contract_address();
        let implementation_address = create_contract_address();
        let contract_abi = Some(create_contract_abi());
        let who = 1;
        set_permission!(who);

        let proxy_contract = Contract::Proxy(ProxyContract {
            details: ContractDetails {
                source: source.clone(),
                address: proxy_address.clone(),
                abi: contract_abi.clone(),
                starting_block: Some(100),
                target_schema: Some(
                    BoundedVec::try_from("test".as_bytes().to_vec()).expect("should always work"),
                ),
                contract_name: None,
                event_details: None,
                ddl_statement: Some(
                    BoundedVec::try_from("test".as_bytes().to_vec()).expect("should always work"),
                ),
            },
            implementation: ImplementationContract {
                details: ContractDetails {
                    source: source.clone(),
                    address: implementation_address.clone(),
                    abi: contract_abi.clone(),
                    starting_block: Some(90),
                    target_schema: None,
                    contract_name: None,
                    event_details: None,
                    ddl_statement: None,
                },
            },
        });

        // Call the extrinsic
        assert_ok!(SmartContracts::add_smartcontract(
            RuntimeOrigin::signed(who),
            proxy_contract.clone(),
            Default::default(),
        ));

        // Verify storage
        assert_eq!(
            ContractStorage::<Test>::get(source.clone(), proxy_address.clone()),
            Some(proxy_contract)
        );

        // Verify event emitted
        System::assert_has_event(
            Event::SmartContractAdded {
                owner: Some(who),
                source,
                address: proxy_address,
            }
            .into(),
        );
    });
}

/// **Test: Adding the Same Smart Contract Twice Fails**
#[test]
fn add_existing_smartcontract_fails() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let source = Source::default();
        let contract_address = create_contract_address();
        let contract_abi = Some(create_contract_abi());
        let who = 1;
        set_permission!(who);

        let normal_contract = Contract::Normal(NormalContract {
            details: ContractDetails {
                source: source.clone(),
                address: contract_address.clone(),
                abi: contract_abi.clone(),
                starting_block: Some(100),
                target_schema: Some(
                    BoundedVec::try_from("test".as_bytes().to_vec()).expect("should always work"),
                ),
                contract_name: None,
                event_details: None,
                ddl_statement: Some(
                    BoundedVec::try_from("test".as_bytes().to_vec()).expect("should always work"),
                ),
            },
        });

        // Insert contract initially
        assert_ok!(SmartContracts::add_smartcontract(
            RuntimeOrigin::signed(who),
            normal_contract.clone(),
            Default::default(),
        ));

        // Attempt to insert the same contract again
        assert_noop!(
            SmartContracts::add_smartcontract(
                RuntimeOrigin::signed(who),
                normal_contract.clone(),
                Default::default(),
            ),
            Error::<Test>::ExistingContractError
        );
    });
}

/// **Test: Removing a Smart Contract Works**
#[test]
fn remove_smartcontract_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let source = Source::default();
        let contract_address = create_contract_address();
        let contract_abi = Some(create_contract_abi());
        let who = 1;
        set_permission!(who);

        let normal_contract = Contract::Normal(NormalContract {
            details: ContractDetails {
                source: source.clone(),
                address: contract_address.clone(),
                abi: contract_abi.clone(),
                starting_block: Some(100),
                target_schema: Some(
                    BoundedVec::try_from("test".as_bytes().to_vec()).expect("should always work"),
                ),
                contract_name: None,
                event_details: None,
                ddl_statement: Some(
                    BoundedVec::try_from("test".as_bytes().to_vec()).expect("should always work"),
                ),
            },
        });

        // Insert contract first
        ContractStorage::<Test>::insert(source.clone(), contract_address.clone(), normal_contract);

        // Ensure it exists
        assert!(ContractStorage::<Test>::contains_key(
            &source,
            &contract_address
        ));

        // Call the extrinsic
        assert_ok!(SmartContracts::remove_smartcontract(
            RuntimeOrigin::signed(who),
            source.clone(),
            contract_address.clone(),
        ));

        // Verify removal
        assert!(!ContractStorage::<Test>::contains_key(
            &source,
            &contract_address
        ));

        // Verify event emitted
        System::assert_last_event(
            Event::SmartContractRemoved {
                owner: Some(who),
                source,
                address: contract_address,
            }
            .into(),
        );
    });
}

/// **Test: Removing a Nonexistent Smart Contract Does Not Fail**
#[test]
fn remove_nonexistent_smartcontract_does_not_fail() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let source = Source::default();
        let contract_address = create_contract_address();
        let who = 1;
        set_permission!(who);

        // Ensure it doesn't exist
        assert!(!ContractStorage::<Test>::contains_key(
            &source,
            &contract_address
        ));

        // Call the extrinsic
        assert_ok!(SmartContracts::remove_smartcontract(
            RuntimeOrigin::signed(who),
            source.clone(),
            contract_address.clone(),
        ));

        // Storage should still be empty
        assert!(!ContractStorage::<Test>::contains_key(
            &source,
            &contract_address
        ));
    });
}
