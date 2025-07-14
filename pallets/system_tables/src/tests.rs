use frame_support::assert_ok;
use frame_system::RawOrigin;
use on_chain_table::OnChainTable;
use sp_core::crypto::AccountId32;
use sp_core::U256;
use sp_runtime::traits::StaticLookup;
use sp_runtime::{DispatchError, Perbill, TokenError};
use sp_staking::StakingInterface;
use sxt_core::parse::{
    StakingSystemRequest,
    SystemFieldValue,
    SystemRequest,
    SystemRequestType,
    SystemTableField,
};
use sxt_core::tables::TableIdentifier;
use sxt_core::utils::{convert_account_id, eth_address_to_substrate_account_id};

use crate::mock::*;
use crate::Pallet;

// Example SCALE encoded Session keys from calling author_rotateKeys() on Alice
const ALICE_SESSION_KEYS: &str = "3084486e870e12fc551eacc173291f0d75ac5fed823aeb1e158bc98db215936202a555f88490d19f7fbacac7078fc87886084efd8227187a73ad05aee6da8ad38edd8739daa5689e9e118eb3be0330bbf80a30ad7639d4f0d70970dbccff9c4a";
const ETH_TEST_WALLET: &str = "44bCf7001D9C3fe8b7aA2BBaaf1B94410db31f5c";
const EXPECTED_TRANSFORMED_ETH_TEST_WALLET_HEX: &str =
    "00000000000000000000000044bCf7001D9C3fe8b7aA2BBaaf1B94410db31f5c";
fn get_staked_message(wallet: &str, amount: U256) -> SystemRequest {
    let wallet_bytes = hex::decode(wallet).unwrap();
    SystemRequest {
        request_type: SystemRequestType::Staking(StakingSystemRequest::Stake),
        table_id: TableIdentifier::from_str_unchecked("STAKED", "SXT_SYSTEM_STAKING"),
        fields: vec![
            SystemTableField::with_value(
                "STAKER".to_string(),
                SystemFieldValue::Bytes(wallet_bytes),
            ),
            SystemTableField::with_value("AMOUNT".to_string(), SystemFieldValue::Decimal(amount)),
        ],
    }
}

fn get_initiate_unstake_message(wallet: &str, amount: U256) -> SystemRequest {
    let wallet_bytes = hex::decode(wallet).unwrap();
    SystemRequest {
        request_type: SystemRequestType::Staking(StakingSystemRequest::UnstakeInitiated),
        table_id: TableIdentifier::from_str_unchecked("UNSTAKEINITIATED", "SXT_SYSTEM_STAKING"),
        fields: vec![
            SystemTableField::with_value(
                "STAKER".to_string(),
                SystemFieldValue::Bytes(wallet_bytes),
            ),
            SystemTableField::with_value("AMOUNT".to_string(), SystemFieldValue::Decimal(amount)),
        ],
    }
}

fn get_cancel_unstake_message(wallet: &str) -> SystemRequest {
    let wallet_bytes = hex::decode(wallet).unwrap();
    SystemRequest {
        request_type: SystemRequestType::Staking(StakingSystemRequest::UnstakeCancelled),
        table_id: TableIdentifier::from_str_unchecked("UNSTAKECANCELLED", "SXT_SYSTEM_STAKING"),
        fields: vec![SystemTableField::with_value(
            "STAKER".to_string(),
            SystemFieldValue::Bytes(wallet_bytes),
        )],
    }
}

fn get_claim_unstake_message(wallet: &str) -> SystemRequest {
    let wallet_bytes = hex::decode(wallet).unwrap();
    SystemRequest {
        request_type: SystemRequestType::Staking(StakingSystemRequest::UnstakeClaimed),
        table_id: TableIdentifier::from_str_unchecked("UNSTAKECLAIMED", "SXT_SYSTEM_STAKING"),
        fields: vec![SystemTableField::with_value(
            "STAKER".to_string(),
            SystemFieldValue::Bytes(wallet_bytes),
        )],
    }
}

fn get_unstaked_message(wallet: &str, amount: U256) -> SystemRequest {
    let wallet_bytes = hex::decode(wallet).unwrap();
    SystemRequest {
        request_type: SystemRequestType::Staking(StakingSystemRequest::Unstaked),
        table_id: TableIdentifier::from_str_unchecked("UNSTAKED", "SXT_SYSTEM_STAKING"),
        fields: vec![
            SystemTableField::with_value(
                "STAKER".to_string(),
                SystemFieldValue::Bytes(wallet_bytes),
            ),
            SystemTableField::with_value("AMOUNT".to_string(), SystemFieldValue::Decimal(amount)),
        ],
    }
}

fn get_register_keys_message(eth_wallet: &str, session_keys: &str, nonce: U256) -> SystemRequest {
    use sxt_core::tables::TableIdentifier;

    // Build the fields for our internal request object
    let sender_field = SystemTableField::with_value(
        "SENDER".to_string(),
        SystemFieldValue::Bytes(hex::decode(eth_wallet).unwrap()),
    );
    let body_field = SystemTableField::with_value(
        "BODY".to_string(),
        SystemFieldValue::Bytes(hex::decode(session_keys).unwrap()),
    );
    let nonce_field =
        SystemTableField::with_value("NONCE".to_string(), SystemFieldValue::Decimal(nonce));

    SystemRequest {
        request_type: SystemRequestType::Message,
        table_id: TableIdentifier::from_str_unchecked("MESSAGE", "SXT_SYSTEM_STAKING"),
        fields: vec![sender_field, body_field, nonce_field],
    }
}

#[test]
fn bonding_with_an_account_works() {
    new_test_ext().execute_with(|| {
        let test_amount = 100;
        // Create a message to stake 100 using the ethereum address
        let bonding = get_staked_message(ETH_TEST_WALLET, test_amount.into());

        // Now we do lookups based on the converted address to assure that state is set correctly
        let transformed_eth_wallet =
            eth_address_to_substrate_account_id::<Test>(ETH_TEST_WALLET).unwrap();

        let expect_hex = hex::decode(EXPECTED_TRANSFORMED_ETH_TEST_WALLET_HEX).unwrap();
        let expected_id = AccountId32::new(expect_hex.try_into().unwrap());
        let converted_hex = convert_account_id::<Test>(expected_id).unwrap();
        assert_eq!(transformed_eth_wallet, converted_hex);

        // Process the staking request
        assert_ok!(crate::process_staking::<Test>(bonding));

        assert_eq!(
            pallet_staking::Pallet::<Test>::bonded(&transformed_eth_wallet),
            Some(transformed_eth_wallet.clone())
        );
        assert_eq!(
            pallet_staking::Pallet::<Test>::ledger(transformed_eth_wallet.into())
                .unwrap()
                .total,
            test_amount
        );
    });
}

#[test]
fn bonding_extra_with_an_account_works() {
    new_test_ext().execute_with(|| {
        crate::mock::run_to_block(1);
        let test_amount = 100;
        // Create a message to stake 100 using the ethereum address
        let bonding = get_staked_message(ETH_TEST_WALLET, test_amount.into());

        // Now we do lookups based on the converted address to assure that state is set correctly
        let transformed_eth_wallet =
            eth_address_to_substrate_account_id::<Test>(ETH_TEST_WALLET).unwrap();

        let expect_hex = hex::decode(EXPECTED_TRANSFORMED_ETH_TEST_WALLET_HEX).unwrap();
        let expected_id = AccountId32::new(expect_hex.try_into().unwrap());
        let converted_hex = convert_account_id::<Test>(expected_id).unwrap();
        assert_eq!(transformed_eth_wallet, converted_hex);

        // Process the staking request
        assert_ok!(crate::process_staking::<Test>(bonding.clone()));

        // Make sure there are no errors in the events
        let events = System::events();
        if let Some(RuntimeEvent::SystemTables(crate::Event::MessageProcessingError { error })) =
            events.last().map(|e| &e.event)
        {
            panic!("Expected no errors!");
        }

        assert_eq!(
            pallet_staking::Pallet::<Test>::bonded(&transformed_eth_wallet),
            Some(transformed_eth_wallet.clone())
        );
        assert_eq!(
            pallet_staking::Pallet::<Test>::ledger(transformed_eth_wallet.clone().into())
                .unwrap()
                .total,
            test_amount
        );

        // Go to the next block
        crate::mock::run_to_block(2);

        // Now bond an additional amount
        assert_ok!(crate::process_staking::<Test>(bonding));

        // Make sure there are no errors in the events
        let events = System::events();
        if let Some(RuntimeEvent::SystemTables(crate::Event::MessageProcessingError { error })) =
            events.last().map(|e| &e.event)
        {
            panic!("Expected no errors! Got: {:?}", error);
        }

        // Check that the amount is increased
        assert_eq!(
            pallet_staking::Pallet::<Test>::ledger(transformed_eth_wallet.into())
                .unwrap()
                .total,
            test_amount * 2
        );
    });
}

#[test]
fn set_session_keys_works_if_stash_is_bonded() {
    new_test_ext().execute_with(|| {
        // We have to bond an amount to establish the stash/controller accounts
        let test_amount = 100;
        let bonding = get_staked_message(ETH_TEST_WALLET, test_amount.into());
        assert_ok!(crate::process_staking::<Test>(bonding));

        // Test registering Alice's Keys
        let request = get_register_keys_message(ETH_TEST_WALLET, ALICE_SESSION_KEYS, U256::from(1));
        assert_ok!(crate::process_evm_message::<Test>(request));

        let wallet = eth_address_to_substrate_account_id::<Test>(ETH_TEST_WALLET).unwrap();
        assert!(pallet_staking::Validators::<Test>::contains_key(&wallet));
        assert!(pallet_session::NextKeys::<Test>::contains_key(&wallet));
    });
}

#[test]
fn registering_keys_without_bonding_first_causes_error() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        // Test registering Alice's Keys
        let request = get_register_keys_message(ETH_TEST_WALLET, ALICE_SESSION_KEYS, U256::from(1));

        assert_ok!(crate::process_evm_message::<Test>(request));

        let events = System::events();
        match events.last().map(|e| &e.event) {
            Some(RuntimeEvent::SystemTables(crate::Event::MessageProcessingError { error })) => {
                assert_eq!(
                    error,
                    &DispatchError::from(pallet_staking::Error::<Test>::NotStash)
                );
            }
            _ => panic!("Expected MessageProcessingError event not found"),
        }
    });
}

#[test]
fn initiating_unstake_works_when_user_is_staked() {
    new_test_ext().execute_with(|| {
        crate::mock::run_to_block(1);

        let test_amount = 100;
        // Create a message to stake 100 using the ethereum address
        let bonding = get_staked_message(ETH_TEST_WALLET, test_amount.into());

        // Now we do lookups based on the converted address to assure that state is set correctly
        let transformed_eth_wallet =
            eth_address_to_substrate_account_id::<Test>(ETH_TEST_WALLET).unwrap();

        // Process the staking request
        assert_ok!(crate::process_staking::<Test>(bonding));

        assert_eq!(
            pallet_staking::Pallet::<Test>::bonded(&transformed_eth_wallet),
            Some(transformed_eth_wallet.clone())
        );
        assert_eq!(
            pallet_staking::Pallet::<Test>::ledger(transformed_eth_wallet.clone().into())
                .unwrap()
                .total,
            test_amount
        );

        crate::mock::run_to_block(100);

        let initiate_unstake = get_initiate_unstake_message(ETH_TEST_WALLET, test_amount.into());

        // Call Unstake
        assert_ok!(crate::process_unstake_initiated::<Test>(initiate_unstake));

        // Ensure than an unbonded event has occurred for the user
        System::assert_has_event(RuntimeEvent::Staking(pallet_staking::Event::Unbonded {
            stash: transformed_eth_wallet,
            amount: test_amount,
        }));
    });
}

#[test]
fn initiating_unstake_fails_when_user_is_not_staked() {
    new_test_ext().execute_with(|| {
        crate::mock::run_to_block(1);
        let test_amount = 100;

        let initiate_unstake = get_initiate_unstake_message(ETH_TEST_WALLET, test_amount.into());

        // Call Unstake. This should succeed, but an error event should be emitted
        assert_ok!(crate::process_unstake_initiated::<Test>(initiate_unstake));

        System::assert_has_event(RuntimeEvent::SystemTables(
            crate::Event::MessageProcessingError {
                error: pallet_staking::Error::<Test>::NotController.into(),
            },
        ));
    });
}

#[test]
fn unstaked_users_can_cancel_unstaking_in_progress() {
    new_test_ext().execute_with(|| {
        crate::mock::run_to_block(1);
        let test_amount = 100;
        // Create a message to stake 100 using the ethereum address
        let bonding = get_staked_message(ETH_TEST_WALLET, test_amount.into());

        // Now we do lookups based on the converted address to assure that state is set correctly
        let transformed_eth_wallet =
            eth_address_to_substrate_account_id::<Test>(ETH_TEST_WALLET).unwrap();

        // Process the staking request
        assert_ok!(crate::process_staking::<Test>(bonding));

        assert_eq!(
            pallet_staking::Pallet::<Test>::bonded(&transformed_eth_wallet),
            Some(transformed_eth_wallet.clone())
        );
        assert_eq!(
            pallet_staking::Pallet::<Test>::ledger(transformed_eth_wallet.clone().into())
                .unwrap()
                .total,
            test_amount
        );
        System::assert_has_event(RuntimeEvent::Staking(pallet_staking::Event::Bonded {
            stash: transformed_eth_wallet.clone(),
            amount: test_amount,
        }));

        System::reset_events();

        crate::mock::run_to_block(100);

        let initiate_unstake = get_initiate_unstake_message(ETH_TEST_WALLET, test_amount.into());

        // Call Unstake
        assert_ok!(crate::process_unstake_initiated::<Test>(initiate_unstake));

        // Ensure than an unbonded event has occurred for the user
        System::assert_has_event(RuntimeEvent::Staking(pallet_staking::Event::Unbonded {
            stash: transformed_eth_wallet.clone(),
            amount: test_amount,
        }));

        System::reset_events();

        // Cancel at the next block
        crate::mock::run_to_block(101);

        let cancel_unstake = get_cancel_unstake_message(ETH_TEST_WALLET);
        assert_ok!(crate::process_unstake_cancelled::<Test>(cancel_unstake));

        // Make sure the total was rebonded
        System::assert_has_event(RuntimeEvent::Staking(pallet_staking::Event::Bonded {
            stash: transformed_eth_wallet.clone(),
            amount: test_amount,
        }));

        // Double check that the staking pallet knows this user is no longer unbonding
        let is_unbonding =
            pallet_staking::Pallet::<Test>::is_unbonding(&transformed_eth_wallet).unwrap();
        assert!(!is_unbonding);
    });
}

#[test]
fn unstaking_works_end_to_end_and_reduces_balance_when_claimed() {
    new_test_ext().execute_with(|| {
        // Start at era 1
        crate::mock::start_active_era(1);
        System::reset_events();

        let test_amount = 100;
        // Create a message to stake 100 using the ethereum address
        let bonding = get_staked_message(ETH_TEST_WALLET, test_amount.into());

        // Now we do lookups based on the converted address to assure that state is set correctly
        let transformed_eth_wallet =
            eth_address_to_substrate_account_id::<Test>(ETH_TEST_WALLET).unwrap();

        // Process the staking request
        assert_ok!(crate::process_staking::<Test>(bonding));

        // Make sure the staking suceeded and events were emitted as expected
        assert_eq!(
            pallet_staking::Pallet::<Test>::bonded(&transformed_eth_wallet),
            Some(transformed_eth_wallet.clone())
        );
        assert_eq!(
            pallet_staking::Pallet::<Test>::ledger(transformed_eth_wallet.clone().into())
                .unwrap()
                .total,
            test_amount
        );
        System::assert_has_event(RuntimeEvent::Staking(pallet_staking::Event::Bonded {
            stash: transformed_eth_wallet.clone(),
            amount: test_amount,
        }));

        // Now go to one era in the future
        crate::mock::start_active_era(2);
        System::reset_events();

        let initiate_unstake = get_initiate_unstake_message(ETH_TEST_WALLET, test_amount.into());

        // Call Unstake
        assert_ok!(crate::process_unstake_initiated::<Test>(initiate_unstake));

        // Assert than an unbonded event has occurred for the user in the staking pallet
        System::assert_has_event(RuntimeEvent::Staking(pallet_staking::Event::Unbonded {
            stash: transformed_eth_wallet.clone(),
            amount: test_amount,
        }));

        // Jump forward by one whole bonding duration + 1 era
        let duration_in_eras = <Test as pallet_staking::Config>::BondingDuration::get() + 7;
        crate::mock::start_active_era(current_era() + duration_in_eras + 1);

        assert!(current_era() > duration_in_eras);
        System::reset_events();

        let claim_unstake = get_claim_unstake_message(ETH_TEST_WALLET);
        assert_ok!(crate::process_unstake_claimed::<Test>(claim_unstake));

        // Ensure the claim event has been emitted
        System::assert_has_event(RuntimeEvent::SystemTables(crate::Event::UnstakingClaimed {
            claimer: transformed_eth_wallet.clone(),
        }));

        // Confirm the unstake as if the contract did
        let unstaked = get_unstaked_message(ETH_TEST_WALLET, test_amount.into());
        assert_ok!(crate::process_unstaked::<Test>(unstaked));

        //Ensure we can see the token burn
        System::assert_has_event(RuntimeEvent::Balances(pallet_balances::Event::Burned {
            who: transformed_eth_wallet,
            amount: test_amount,
        }));
    });
}

#[test]
fn attempting_to_claim_before_unbonding_period_produces_error() {
    new_test_ext().execute_with(|| {
        crate::mock::start_active_era(1);
        let test_amount = 100;
        // Create a message to stake 100 using the ethereum address
        let bonding = get_staked_message(ETH_TEST_WALLET, test_amount.into());

        // Now we do lookups based on the converted address to assure that state is set correctly
        let transformed_eth_wallet =
            eth_address_to_substrate_account_id::<Test>(ETH_TEST_WALLET).unwrap();

        // Process the staking request
        assert_ok!(crate::process_staking::<Test>(bonding));

        assert_eq!(
            pallet_staking::Pallet::<Test>::bonded(&transformed_eth_wallet),
            Some(transformed_eth_wallet.clone())
        );
        assert_eq!(
            pallet_staking::Pallet::<Test>::ledger(transformed_eth_wallet.clone().into())
                .unwrap()
                .total,
            test_amount
        );
        System::assert_has_event(RuntimeEvent::Staking(pallet_staking::Event::Bonded {
            stash: transformed_eth_wallet.clone(),
            amount: test_amount,
        }));

        System::reset_events();
        crate::mock::start_active_era(2);

        let initiate_unstake = get_initiate_unstake_message(ETH_TEST_WALLET, test_amount.into());

        // Call Unstake
        assert_ok!(crate::process_unstake_initiated::<Test>(initiate_unstake));

        // Try to claim at the very next era
        crate::mock::start_active_era(3);
        System::reset_events();

        let claim_unstake = get_claim_unstake_message(ETH_TEST_WALLET);

        assert_ok!(crate::process_unstake_claimed::<Test>(claim_unstake));

        // Ensure there was an error emitted
        System::assert_has_event(RuntimeEvent::SystemTables(
            crate::Event::MessageProcessingError {
                error: crate::Error::<Test>::FundsLocked.into(),
            },
        ));

        System::reset_events();

        // Make sure that even if we call unstake, we don't get the balance back
        let unstaked = get_unstaked_message(ETH_TEST_WALLET, test_amount.into());
        assert_ok!(crate::process_unstaked::<Test>(unstaked));
    });
}

#[test]
fn nonce_increments_on_successful_messages() {
    new_test_ext().execute_with(|| {
        let eth_sender = eth_address_to_substrate_account_id::<Test>(ETH_TEST_WALLET).unwrap();

        // We have to bond an amount to establish the stash/controller accounts
        let test_amount = 100;
        let bonding = get_staked_message(ETH_TEST_WALLET, test_amount.into());
        assert_ok!(crate::process_staking::<Test>(bonding));

        // Now try to register
        let first_nonce = U256::from(1);
        let request = get_register_keys_message(ETH_TEST_WALLET, ALICE_SESSION_KEYS, first_nonce);
        assert_ok!(crate::process_evm_message::<Test>(request.clone()));

        // The last processed should be 1 now
        assert_eq!(
            crate::LastProcessedUserNonce::<Test>::get(&eth_sender),
            Some(first_nonce)
        );

        // Send another valid message with a higher nonce
        let next_nonce = U256::from(2);
        let request = get_register_keys_message(ETH_TEST_WALLET, ALICE_SESSION_KEYS, next_nonce);
        assert_ok!(crate::process_evm_message::<Test>(request));

        // Ensure the last processed is now 2
        assert_eq!(
            crate::LastProcessedUserNonce::<Test>::get(&eth_sender),
            Some(next_nonce)
        );
    });
}

#[test]
fn message_with_duplicate_nonce_should_fail() {
    new_test_ext().execute_with(|| {
        let eth_sender = eth_address_to_substrate_account_id::<Test>(ETH_TEST_WALLET).unwrap();

        // We have to bond an amount to establish the stash/controller accounts
        let test_amount = 100;
        let bonding = get_staked_message(ETH_TEST_WALLET, test_amount.into());
        assert_ok!(crate::process_staking::<Test>(bonding));

        // Now try to register
        let test_nonce = U256::from(1);
        let request = get_register_keys_message(ETH_TEST_WALLET, ALICE_SESSION_KEYS, U256::from(1));
        assert_ok!(crate::process_evm_message::<Test>(request.clone()));

        // The last processed should be 1 now
        assert_eq!(
            crate::LastProcessedUserNonce::<Test>::get(&eth_sender),
            Some(test_nonce)
        );

        // Send the same message, which should succeed but emit an error event and not be
        // processed
        assert_ok!(crate::process_evm_message::<Test>(request));

        // Ensure the last processed is still 1
        assert_eq!(
            crate::LastProcessedUserNonce::<Test>::get(eth_sender),
            Some(test_nonce)
        );
    });
}

#[test]
fn message_with_a_future_nonce_should_fail() {
    new_test_ext().execute_with(|| {
        let eth_sender = eth_address_to_substrate_account_id::<Test>(ETH_TEST_WALLET).unwrap();
        // We have to bond an amount to establish the stash/controller accounts
        let test_amount = 100;
        let bonding = get_staked_message(ETH_TEST_WALLET, test_amount.into());
        assert_ok!(crate::process_staking::<Test>(bonding));

        let expected_nonce = U256::from(0);
        assert_eq!(
            crate::LastProcessedUserNonce::<Test>::get(&eth_sender),
            None
        );

        // Now try to register with a nonce that is 2 in the future
        let request =
            get_register_keys_message(ETH_TEST_WALLET, ALICE_SESSION_KEYS, expected_nonce + 2);

        // The message should succeed, but emit an error event and not get processed
        assert_ok!(crate::process_evm_message::<Test>(request.clone()));

        // Ensure the last processed nonce has not changed
        assert_eq!(
            crate::LastProcessedUserNonce::<Test>::get(&eth_sender),
            None
        );
    });
}

#[test]
fn nomination_parsed_from_ipc_record_batch_works() {
    use std::io::Cursor;

    use arrow::ipc::reader::StreamReader;
    use sxt_core::tables::TableIdentifier;

    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let eth_address = "0x5aa92cbe0a15ee0cd66022f957606fedcb30067b";
        let account = eth_address_to_substrate_account_id::<Test>(eth_address).unwrap();

        let amount = 1_000_000_000_000_000u128;
        let lookup = <Test as frame_system::Config>::Lookup::unlookup(account.clone());

        assert_ok!(pallet_balances::Pallet::<Test>::force_set_balance(
            RawOrigin::Root.into(),
            lookup,
            amount
        ));

        assert_ok!(pallet_staking::Pallet::<Test>::bond(
            RawOrigin::Signed(account.clone()).into(),
            amount,
            pallet_staking::RewardDestination::Staked
        ));

        // Step 2: Decode the record batch from the provided hex
        let hex_data = "ffffffffb80200001000000000000a000e000c000b0004000a000000140000000000000104000a000c000000080004000a00000008000000bc000000020000006400000004000000acffffff08000000340000002a000000307837623363626161666538666633636266343535333839336664636164386435633436646239306162000010000000636f6e74726163745f616464726573730000000008000c000800040008000000080000003400000029000000626c6f636b5f6e756d6265727c7472616e73616374696f6e5f686173687c6576656e745f696e6465780000000c0000007072696d6172795f6b65797300000000070000006c01000018010000dc000000a40000006c0000003400000004000000c0feffff140000000c000000000000040c0000000000000038ffffff090000006e6f6d696e61746f72000000ecfeffff140000000c000000000000050c0000000000000064ffffff130000006e6f646573656432353531397075626b6579730020ffffff140000000c000000000000040c0000000000000098ffffff10000000636f6e74726163745f616464726573730000000054ffffff1000000018000000000000021400000044ffffff2000000000000001000000000b0000006576656e745f696e6465780088ffffff180000000c0000000000000410000000000000000400040004000000100000007472616e73616374696f6e5f6861736800000000c0ffffff1c0000000c0000000000000a20000000000000000000060008000600060000000000010000000000000000000a00000074696d655f7374616d70000010001400100000000f00040000000800100000001800000020000000000000021c00000008000c0004000b00080000004000000000000001000000000c000000626c6f636b5f6e756d6265720000000000000000000000000000000000000000000000000000000000000000fffffffff8010000100000000c001a0018001700040008000c00000020000000c004000000000000000000000000000304000a0018000c00080004000a0000008c0000001000000001000000000000000000000007000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000001000000000000000000000000000000010000000000000000000000000000000000000012000000000000000000000001000000000000004000000000000000080000000000000080000000000000000100000000000000c0000000000000000800000000000000000100000000000001000000000000004001000000000000080000000000000080010000000000002000000000000000c0010000000000000100000000000000000200000000000004000000000000004002000000000000010000000000000080020000000000000800000000000000c002000000000000140000000000000000030000000000000100000000000000400300000000000008000000000000008003000000000000460000000000000000040000000000000100000000000000400400000000000008000000000000008004000000000000140000000000000000000000000000000000000000000000ff000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a3797d00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ff000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a01c4584960100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ff000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e69728b556f9c4edbe7627bdbe8ef58c6261443455f191b0835a1e536e478a20000000000000000000000000000000000000000000000000000000000000000ff0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000003c000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ff000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007b3cbaafe8ff3cbf4553893fdcad8d5c46db90ab0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ff000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004600000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000005b22307861633061323832306537393266623533656439396337303838343730306365316166336130663664666166346634383461363335343266626438353437613265225d00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ff000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000005aa92cbe0a15ee0cd66022f957606fedcb30067b0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ffffffff00000000"; // move long hex to a separate file for readability
        let raw_bytes = hex::decode(hex_data).expect("Valid hex");
        let cursor = Cursor::new(raw_bytes);
        let mut reader = StreamReader::try_new(cursor, None).expect("valid IPC stream");
        let batch = reader.next().expect("1st batch").expect("non-empty");

        // Step 3: Convert RecordBatch -> OnChainTable
        let oc_table = OnChainTable::try_from(batch).expect("valid OnChainTable");

        // Step 4: Table identifier for nominate
        let table_id = TableIdentifier::from_str_unchecked("NOMINATED", "SXT_SYSTEM_STAKING");

        // Step 5: Run it through the pallet
        assert_ok!(Pallet::<Test>::process_system_table(
            table_id.clone(),
            oc_table
        ));

        let found =
            pallet_staking::Nominators::<Test>::drain().any(|(nominator, _)| nominator == account);

        assert!(found, "Expected nominator not found in staking nominators");
    });
}

#[test]
fn default_commission_is_10_percent() {
    new_test_ext().execute_with(|| {
        let eth_sender = eth_address_to_substrate_account_id::<Test>(ETH_TEST_WALLET).unwrap();

        // We have to bond an amount to establish the stash/controller accounts
        let test_amount = 100;
        let bonding = get_staked_message(ETH_TEST_WALLET, test_amount.into());
        assert_ok!(crate::process_staking::<Test>(bonding));

        // Now try to register
        let first_nonce = U256::from(1);
        let request = get_register_keys_message(ETH_TEST_WALLET, ALICE_SESSION_KEYS, first_nonce);
        assert_ok!(crate::process_evm_message::<Test>(request.clone()));

        // The last processed should be 1 now
        assert_eq!(
            crate::LastProcessedUserNonce::<Test>::get(&eth_sender),
            Some(first_nonce)
        );

        assert_eq!(
            pallet_staking::Pallet::<Test>::bonded(&eth_sender),
            Some(eth_sender.clone())
        );
        assert_eq!(
            pallet_staking::Pallet::<Test>::ledger(eth_sender.clone().into())
                .unwrap()
                .total,
            test_amount
        );

        // Check that the commission is 10%
        let prefs = pallet_staking::Pallet::<Test>::validators(&eth_sender);
        assert_eq!(prefs.commission, Perbill::from_percent(10));
    });
}

#[test]
fn changing_eras_works() {
    new_test_ext().execute_with(|| {
        crate::mock::start_active_era(1);
        assert_eq!(crate::mock::active_era(), 1);
        crate::mock::start_active_era(2);
    });
}
