// use crate::messages::RegisterSessionMessage;
use frame_support::{assert_err, assert_ok};
use pallet_staking::Validators;
use sp_core::U256;
use sxt_core::tables::TableIdentifier;
use sxt_core::utils::eth_address_to_substrate_account_id;

use crate::mock::*;
use crate::parse::{
    StakingSystemRequest,
    SystemFieldValue,
    SystemRequest,
    SystemRequestType,
    SystemTableField,
};

// Example SCALE encoded Session keys from calling author_rotateKeys() on Alice
const ALICE_SESSION_KEYS: &str = "0x3084486e870e12fc551eacc173291f0d75ac5fed823aeb1e158bc98db215936202a555f88490d19f7fbacac7078fc87886084efd8227187a73ad05aee6da8ad38edd8739daa5689e9e118eb3be0330bbf80a30ad7639d4f0d70970dbccff9c4a";
const ETH_TEST_WALLET: &str = "44bCf7001D9C3fe8b7aA2BBaaf1B94410db31f5c";

fn get_staked_message(wallet: &str, amount: U256) -> SystemRequest {
    SystemRequest {
        request_type: SystemRequestType::Staking(StakingSystemRequest::Stake),
        table_id: TableIdentifier::from_str_unchecked("STAKED", "SXT_SYSTEM_STAKING"),
        fields: vec![
            SystemTableField::with_value(
                "STAKER".to_string(),
                SystemFieldValue::Varchar(wallet.to_string()),
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
        SystemFieldValue::Varchar(ETH_TEST_WALLET.to_string()),
    );
    let message_field = SystemTableField::with_value(
        "MESSAGE".to_string(),
        SystemFieldValue::Varchar(session_keys.to_string()),
    );
    let nonce_field =
        SystemTableField::with_value("NONCE".to_string(), SystemFieldValue::Decimal(nonce));

    SystemRequest {
        request_type: SystemRequestType::Message,
        table_id: TableIdentifier::from_str_unchecked("MESSAGE", "SXT_SYSTEM_STAKING"),
        fields: vec![sender_field, message_field, nonce_field],
    }
}

#[test]
fn bonding_with_an_account_works() {
    new_test_ext().execute_with(|| {
        let test_amount = 100;
        // Create a message to stake 100 using the ethereum address
        let bonding = get_staked_message(ETH_TEST_WALLET, test_amount.into());

        // Process the staking request
        assert_ok!(crate::process_staking::<Test>(bonding));

        // Now we do lookups based on the converted address to assure that state is set correctly
        let transformed_eth_wallet =
            eth_address_to_substrate_account_id::<Test>(ETH_TEST_WALLET).unwrap();
        assert_eq!(
            pallet_staking::Pallet::<Test>::bonded(&transformed_eth_wallet).unwrap(),
            transformed_eth_wallet
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
        assert!(pallet_staking::Validators::<Test>::contains_key(wallet));
        assert!(pallet_session::NextKeys::<Test>::contains_key(wallet));
    });
}

#[test]
fn registering_keys_without_bonding_first_causes_error() {
    new_test_ext().execute_with(|| {
        // Test registering Alice's Keys
        let request = get_register_keys_message(ETH_TEST_WALLET, ALICE_SESSION_KEYS, U256::from(1));
        assert_err!(
            crate::process_evm_message::<Test>(request),
            pallet_staking::Error::<Test>::NotStash
        );
    });
}

#[test]
fn nonce_increments_on_successful_messages() {
    new_test_ext().execute_with(|| {
        // We have to bond an amount to establish the stash/controller accounts
        let test_amount = 100;
        let bonding = get_staked_message(ETH_TEST_WALLET, test_amount.into());
        assert_ok!(crate::process_staking::<Test>(bonding));

        // Now try to register
        let first_nonce = U256::from(1);
        let request = get_register_keys_message(ETH_TEST_WALLET, ALICE_SESSION_KEYS, first_nonce);
        assert_ok!(crate::process_evm_message::<Test>(request.clone()));

        // The last processed should be 1 now
        assert_eq!(crate::LastProcessedNonce::<Test>::get(), first_nonce);

        // Send another valid message with a higher nonce
        let next_nonce = U256::from(2);
        let request = get_register_keys_message(ETH_TEST_WALLET, ALICE_SESSION_KEYS, next_nonce);
        assert_ok!(crate::process_evm_message::<Test>(request));

        // Ensure the last processed is now 2
        assert_eq!(crate::LastProcessedNonce::<Test>::get(), next_nonce);
    });
}

#[test]
fn message_with_duplicate_nonce_should_fail() {
    new_test_ext().execute_with(|| {
        // We have to bond an amount to establish the stash/controller accounts
        let test_amount = 100;
        let bonding = get_staked_message(ETH_TEST_WALLET, test_amount.into());
        assert_ok!(crate::process_staking::<Test>(bonding));

        // Now try to register
        let test_nonce = U256::from(1);
        let request = get_register_keys_message(ETH_TEST_WALLET, ALICE_SESSION_KEYS, U256::from(1));
        assert_ok!(crate::process_evm_message::<Test>(request.clone()));

        // The last processed should be 1 now
        assert_eq!(crate::LastProcessedNonce::<Test>::get(), test_nonce);

        // Send the same message, which should fail because of nonce re-use
        assert_err!(
            crate::process_evm_message::<Test>(request),
            crate::Error::<Test>::LateNonce
        );

        // Ensure the last processed is still 1
        assert_eq!(crate::LastProcessedNonce::<Test>::get(), test_nonce);
    });
}

#[test]
fn message_with_a_future_nonce_should_fail() {
    new_test_ext().execute_with(|| {
        // We have to bond an amount to establish the stash/controller accounts
        let test_amount = 100;
        let bonding = get_staked_message(ETH_TEST_WALLET, test_amount.into());
        assert_ok!(crate::process_staking::<Test>(bonding));

        let expected_nonce = U256::from(0);
        assert_eq!(crate::LastProcessedNonce::<Test>::get(), expected_nonce);

        // Now try to register with a nonce that is 2 in the future
        let request =
            get_register_keys_message(ETH_TEST_WALLET, ALICE_SESSION_KEYS, expected_nonce + 2);
        assert_err!(
            crate::process_evm_message::<Test>(request.clone()),
            crate::Error::<Test>::FutureNonce
        );

        // Ensure the last processed nonce has not changed
        assert_eq!(crate::LastProcessedNonce::<Test>::get(), expected_nonce);
    });
}
