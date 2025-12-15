use frame_support::dispatch::RawOrigin;
use frame_support::{assert_err, assert_ok};
use sp_core::crypto::{AccountId32, Ss58Codec};
use sp_core::U256;
use sp_runtime::DispatchError;
use sxt_core::parse::{
    SystemFieldValue,
    SystemRequest,
    SystemRequestType,
    SystemTableField,
    ZKPayRequest,
};
use sxt_core::tables::TableIdentifier;
use sxt_core::ByteString;

use crate::mock::*;
use crate::{Asset, AssetAddress, ComputeCreditAddress, Event, SupportedAssets};

// Example SCALE encoded Session keys from calling author_rotateKeys() on Alice
const ETH_TEST_WALLET: &str = "44bCf7001D9C3fe8b7aA2BBaaf1B94410db31f5c";
fn get_send_payment_message(
    asset: Vec<u8>,
    amount: U256,
    target: Vec<u8>,
    on_behalf_of: Vec<u8>,
    sender: Vec<u8>,
) -> SystemRequest {
    SystemRequest {
        request_type: SystemRequestType::ZkPay(ZKPayRequest::SendPayment),
        table_id: TableIdentifier::from_str_unchecked("SENDPAYMENT", "SXT_SYSTEM_ZKPAY"),
        fields: vec![
            SystemTableField::with_value("ASSET".to_string(), SystemFieldValue::Bytes(asset)),
            SystemTableField::with_value("AMOUNT".to_string(), SystemFieldValue::Decimal(amount)),
            SystemTableField::with_value(
                "ONBEHALFOF".to_string(),
                SystemFieldValue::Bytes(on_behalf_of),
            ),
            SystemTableField::with_value("TARGET".to_string(), SystemFieldValue::Bytes(target)),
            SystemTableField::with_value(
                "MEMO".to_string(),
                SystemFieldValue::Bytes("???".as_bytes().to_vec()),
            ),
            SystemTableField::with_value(
                "AMOUNTINUSD".to_string(),
                SystemFieldValue::Decimal(amount.checked_div(10.into()).unwrap()),
            ),
            SystemTableField::with_value("SENDER".to_string(), SystemFieldValue::Bytes(sender)),
        ],
    }
}

#[test]
fn non_root_calls_produce_an_error_and_do_not_update_storage() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let sender =
            AccountId32::from_ss58check("5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY")
                .unwrap();
        let test_address =
            ByteString::try_from(hex::decode("27d4d2af364c1ad2ebdb2a28d6cb7b99ede1d450").unwrap())
                .unwrap();
        ComputeCreditAddress::<Test>::put(test_address.clone());

        let test_new_address = hex::decode("deadbeef364c1ad2ebdb2a28d6cb7b99ede1d450").unwrap();

        assert_err!(
            crate::Pallet::<Test>::update_compute_credit_address(
                RuntimeOrigin::signed(sender),
                ByteString::try_from(test_new_address).unwrap()
            ),
            DispatchError::BadOrigin
        );

        assert_eq!(test_address, ComputeCreditAddress::<Test>::get());
    })
}

#[test]
fn update_contract_address_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        //
        let test_old_address =
            ByteString::try_from(hex::decode("deadbeef364c1ad2ebdb2a28d6cb7b99ede1d450").unwrap())
                .unwrap();
        ComputeCreditAddress::<Test>::put(test_old_address.clone());

        let test_address =
            ByteString::try_from(hex::decode("27d4d2af364c1ad2ebdb2a28d6cb7b99ede1d450").unwrap())
                .unwrap();

        let sender = RawOrigin::Root;

        // Send the transaction
        assert_ok!(crate::Pallet::<Test>::update_compute_credit_address(
            sender.into(),
            test_address.clone()
        ));

        // Make sure the on-chain state is updated
        assert_eq!(test_address, ComputeCreditAddress::<Test>::get());

        // Make sure we emitted the expected event
        System::assert_last_event(
            Event::<Test>::ComputeCreditAddressUpdated {
                old_address: test_old_address,
                new_address: test_address,
            }
            .into(),
        );
    })
}

#[test]
fn update_compute_credit_address_rejects_all_zero_address() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let sender = RawOrigin::Root;
        let all_zero_address = ByteString::try_from(vec![0u8; 20]).unwrap();

        // Attempting to set the all-zero address should fail with ContractAddressError
        let result =
            crate::Pallet::<Test>::update_compute_credit_address(sender.into(), all_zero_address);

        assert_err!(result, crate::Error::<Test>::ContractAddressError);
    })
}

#[test]
fn buying_compute_credits_works() {
    new_test_ext().execute_with(|| {
        let test_amount = 100_000;

        // Configure a placeholder compute credit address and zkpay asset metadata
        ComputeCreditAddress::<Test>::put(get_compute_credit_address());
        let test_asset = get_zkpay_asset();
        SupportedAssets::<Test>::set(test_asset.address.clone(), Some(test_asset.clone()));

        // Test Sender
        let sender_bytes = hex::decode(ETH_TEST_WALLET).unwrap();

        // Alice's public key
        let alice_bytes =
            hex::decode("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d")
                .unwrap();
        let alice_ss58 = "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY";
        let alice_account = AccountId32::from_string(alice_ss58).unwrap();

        // Create a message to buy 100 compute credits by ZKpaying the Compute Credit address
        let send_payment = get_send_payment_message(
            test_asset.address.to_vec(),
            test_amount.into(),
            get_compute_credit_address().to_vec(),
            alice_bytes,
            sender_bytes,
        );

        // The account should start with no balance
        let old_balance = pallet_balances::Pallet::<Test>::free_balance(&alice_account);
        assert_eq!(0, old_balance);

        // Process the send_payment
        assert_ok!(crate::Pallet::<Test>::process_send_payment(send_payment));

        // And should now have balance equal to test_amount
        let new_balance = pallet_balances::Pallet::<Test>::free_balance(alice_account);
        assert_eq!(new_balance, test_amount);
    });
}

#[test]
fn buying_compute_credits_with_invalid_on_behalf_of_fails_gracefully() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let test_amount = 100_000;

        // Configure a placeholder compute credit address and zkpay asset metadata
        ComputeCreditAddress::<Test>::put(get_compute_credit_address());
        let test_asset = get_zkpay_asset();
        SupportedAssets::<Test>::set(test_asset.address.clone(), Some(test_asset.clone()));

        // Test Sender
        let sender_bytes = hex::decode(ETH_TEST_WALLET).unwrap();

        // An Invalid On_Behalf_Of address (just UTF-8 bytes)
        let invalid_on_behalf_of_bytes = "invalid-address".as_bytes().to_vec();

        // Create a message to buy 100 compute credits by ZKpaying the Compute Credit address
        let send_payment = get_send_payment_message(
            test_asset.address.to_vec(),
            test_amount.into(),
            get_compute_credit_address().to_vec(),
            invalid_on_behalf_of_bytes,
            sender_bytes,
        );

        // The send payment should still be a valid extrinsic, but will emit an error event
        assert_ok!(crate::Pallet::<Test>::process_send_payment(send_payment));

        // Assert there was an error emitted
        System::assert_last_event(
            Event::<Test>::ZkPayProcessingError {
                error: crate::Error::<Test>::InvalidAccountId.into(),
            }
            .into(),
        );
    });
}

#[test]
fn unsupported_assets_fail_gracefully() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let test_amount = 100_000;

        // Configure a placeholder compute credit address and zkpay asset metadata
        ComputeCreditAddress::<Test>::put(get_compute_credit_address());

        let test_asset = get_zkpay_asset();
        SupportedAssets::<Test>::set(test_asset.address.clone(), Some(test_asset));

        // Test Sender
        let sender_bytes = hex::decode(ETH_TEST_WALLET).unwrap();

        // An Invalid On_Behalf_Of address (just UTF-8 bytes)
        let invalid_on_behalf_of_bytes = "invalid-address".as_bytes().to_vec();

        // Using the wrong asset address here should restult in an error being emitted
        let send_payment = get_send_payment_message(
            "01".as_bytes().to_vec(),
            test_amount.into(),
            get_compute_credit_address().to_vec(),
            invalid_on_behalf_of_bytes,
            sender_bytes,
        );

        // The send payment should still be a valid extrinsic, but will emit an error event
        assert_ok!(crate::Pallet::<Test>::process_send_payment(send_payment));

        // Assert there was an error emitted
        System::assert_last_event(
            Event::<Test>::ZkPayProcessingError {
                error: crate::Error::<Test>::UnsupportedAsset.into(),
            }
            .into(),
        );
    });
}

#[test]
fn updating_supported_asset_works_if_previously_none() {
    new_test_ext().execute_with(|| {
        let test_asset = get_zkpay_asset();

        assert!(SupportedAssets::<Test>::get(test_asset.address.clone()).is_none());

        assert_ok!(crate::Pallet::<Test>::set_supported_asset(
            RuntimeOrigin::root(),
            test_asset.clone()
        ));

        assert_eq!(
            SupportedAssets::<Test>::get(test_asset.address.clone()),
            Some(test_asset)
        );
    });
}

#[test]
fn updating_supported_asset_overwrites_if_previously_some() {
    new_test_ext().execute_with(|| {
        let test_asset = get_zkpay_asset();

        SupportedAssets::<Test>::set(test_asset.address.clone(), Some(test_asset.clone()));
        assert_eq!(
            SupportedAssets::<Test>::get(test_asset.address.clone()),
            Some(test_asset.clone())
        );

        // Change the token decimals, but keep everything else the same
        let mut other_asset = test_asset.clone();
        other_asset.token_decimals = 2;

        assert_ok!(crate::Pallet::<Test>::set_supported_asset(
            RuntimeOrigin::root(),
            other_asset.clone()
        ));

        assert_eq!(
            SupportedAssets::<Test>::get(test_asset.address.clone())
                .unwrap()
                .token_decimals,
            2
        );
    });
}

fn get_compute_credit_address() -> ByteString {
    ByteString::try_from(hex::decode("27d4d2af364c1ad2ebdb2a28d6cb7b99ede1d450").unwrap()).unwrap()
}

fn get_zkpay_asset() -> crate::Asset {
    get_test_asset(sp_runtime::BoundedVec::try_from("FF".as_bytes().to_vec()).unwrap())
}

fn get_test_asset(asset_addr: AssetAddress) -> Asset {
    crate::Asset {
        address: asset_addr,
        allowed_payment_types: sp_runtime::BoundedVec::try_from("00".as_bytes().to_vec()).unwrap(),
        price_feed: sp_runtime::BoundedVec::try_from("FF".as_bytes().to_vec()).unwrap(),
        token_decimals: 18,
        stale_price_threshold_in_seconds: core::time::Duration::from_secs(240),
    }
}
