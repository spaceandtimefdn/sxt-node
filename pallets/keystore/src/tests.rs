use codec::Encode;
use frame_support::{assert_err, assert_ok};
use k256::ecdsa::{SigningKey, VerifyingKey};
use sxt_core::attestation::{sign_eth_message, EthereumSignature, RegisterExternalAddress};
use sxt_core::keystore::{EthereumKey, UnregisterExternalAddress, UserKeystore};

use crate::mock::*;
use crate::{Error, Pallet};

fn create_signed_message_and_keypair(account_id: u64) -> ([u8; 33], EthereumSignature) {
    // Generate a new keypair.
    let (signing_key, verifying_key) = generate_keypair();

    // Encode the account ID as the message.
    let message = account_id.encode();

    // Sign the encoded message using `sign_eth_message`.
    let private_key_bytes = signing_key.to_bytes();
    let signature = sign_eth_message(&private_key_bytes, &message).expect("Failed to sign message");

    // Get the verifying key in SEC1 format.
    let verifying_key_sec1 = &*verifying_key.to_sec1_bytes();
    let verifying_key_sec1: [u8; 33] = verifying_key_sec1.try_into().unwrap();

    // Return the signing key, verifying key (SEC1 format), and signature.
    (verifying_key_sec1, signature)
}

fn generate_keypair() -> (SigningKey, VerifyingKey) {
    use k256::elliptic_curve::rand_core::OsRng;

    let signing_key = SigningKey::random(&mut OsRng);
    (signing_key.clone(), *signing_key.verifying_key())
}

#[test]
fn register_ethereum_key_success() {
    new_test_ext().execute_with(|| {
        let account_id: u64 = 1;
        let (public_key, signature) = create_signed_message_and_keypair(account_id);

        let address20 =
            sxt_core::attestation::uncompressed_public_key_to_address(&public_key).unwrap();

        let registration = RegisterExternalAddress::EthereumAddress {
            signature,
            proposed_pub_key: public_key,
            address20: address20.clone(),
        };

        assert_ok!(Pallet::<Test>::register_key(
            RuntimeOrigin::root(),
            account_id,
            registration
        ));

        let stored_key = Pallet::<Test>::keys(account_id);
        let expected_keystore = UserKeystore {
            eth_key: Some(EthereumKey {
                pub_key: public_key,
                address20,
            }),
        };
        assert_eq!(stored_key, Some(expected_keystore));
    });
}

#[test]
fn register_ethereum_key_fails_if_signature_wrong() {
    new_test_ext().execute_with(|| {
        let account_id: u64 = 1;

        // Generate a keypair and register the key.
        let (public_key, signature) = create_signed_message_and_keypair(account_id);
        let address20 =
            sxt_core::attestation::uncompressed_public_key_to_address(&public_key).unwrap();

        let registration = RegisterExternalAddress::EthereumAddress {
            signature,
            proposed_pub_key: public_key,
            address20: address20.clone(),
        };
        assert_ok!(Pallet::<Test>::register_key(
            RuntimeOrigin::root(),
            account_id,
            registration
        ));

        // Attempt to register the same key for another account.
        let another_account_id: u64 = 2;
        let duplicate_registration = RegisterExternalAddress::EthereumAddress {
            signature,
            proposed_pub_key: public_key,
            address20,
        };
        assert_err!(
            Pallet::<Test>::register_key(
                RuntimeOrigin::root(),
                another_account_id,
                duplicate_registration
            ),
            Error::<Test>::VerificationError
        );
    });
}

#[test]
fn register_ethereum_key_fails_if_account_already_registered() {
    new_test_ext().execute_with(|| {
        let account_id: u64 = 1;

        // Generate and register the first key.
        let (public_key_1, signature_1) = create_signed_message_and_keypair(account_id);
        let address20_1 =
            sxt_core::attestation::uncompressed_public_key_to_address(&public_key_1).unwrap();

        let registration_1 = RegisterExternalAddress::EthereumAddress {
            signature: signature_1,
            proposed_pub_key: public_key_1,
            address20: address20_1.clone(),
        };
        assert_ok!(Pallet::<Test>::register_key(
            RuntimeOrigin::root(),
            account_id,
            registration_1
        ));

        // Generate a second key and attempt to register it for the same account.
        let (public_key_2, signature_2) = create_signed_message_and_keypair(account_id);
        let address20_2 =
            sxt_core::attestation::uncompressed_public_key_to_address(&public_key_2).unwrap();

        let registration_2 = RegisterExternalAddress::EthereumAddress {
            signature: signature_2,
            proposed_pub_key: public_key_2,
            address20: address20_2,
        };
        assert_err!(
            Pallet::<Test>::register_key(RuntimeOrigin::root(), account_id, registration_2),
            Error::<Test>::EthereumKeyAlreadyRegistered
        );
    });
}

#[test]
fn register_ethereum_key_fails_if_signature_invalid() {
    new_test_ext().execute_with(|| {
        let account_id: u64 = 1;

        // Generate a keypair and use an invalid signature.
        let (public_key, _) = create_signed_message_and_keypair(account_id);
        let address20 =
            sxt_core::attestation::uncompressed_public_key_to_address(&public_key).unwrap();

        let invalid_signature = EthereumSignature {
            r: [0u8; 32],
            s: [0u8; 32],
            v: 27,
        };
        let registration = RegisterExternalAddress::EthereumAddress {
            signature: invalid_signature,
            proposed_pub_key: public_key,
            address20,
        };

        assert_err!(
            Pallet::<Test>::register_key(RuntimeOrigin::root(), account_id, registration),
            Error::<Test>::VerificationError
        );
    });
}

#[test]
fn remove_ethereum_key_success() {
    new_test_ext().execute_with(|| {
        let account_id: u64 = 1;
        let (public_key, signature) = create_signed_message_and_keypair(account_id);

        let address20 =
            sxt_core::attestation::uncompressed_public_key_to_address(&public_key).unwrap();

        // Register the key.
        let registration = RegisterExternalAddress::EthereumAddress {
            signature,
            proposed_pub_key: public_key,
            address20: address20.clone(),
        };
        assert_ok!(Pallet::<Test>::register_key(
            RuntimeOrigin::root(),
            account_id,
            registration
        ));

        // Ensure the key is stored.
        assert!(Pallet::<Test>::keys(account_id).is_some());

        let deregistration = UnregisterExternalAddress::EthereumAddress;

        // Remove the key.
        assert_ok!(Pallet::<Test>::unregister_key(
            RuntimeOrigin::root(),
            account_id,
            deregistration,
        ));

        // Ensure the key is no longer stored.
        let keystore = Pallet::<Test>::keys(account_id).unwrap();
        assert!(keystore.eth_key.is_none());
    });
}
