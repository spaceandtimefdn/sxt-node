use codec::Encode;
use frame_support::{assert_err, assert_noop, assert_ok, BoundedVec};
use k256::ecdsa::{SigningKey, VerifyingKey};
use sp_core::H256;
use sp_runtime::DispatchResult;
use sxt_core::attestation::{
    sign_eth_message,
    Attestation,
    AttestationKey,
    EthereumSignature,
    RegisterExternalAddress,
};

use crate::mock::*;
use crate::{Error, Pallet};

fn create_signed_message_and_keypair(account_id: u64) -> (SigningKey, [u8; 33], EthereumSignature) {
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

    (signing_key, verifying_key_sec1, signature)
}

fn generate_keypair() -> (SigningKey, VerifyingKey) {
    use k256::elliptic_curve::rand_core::OsRng;

    let signing_key = SigningKey::random(&mut OsRng);
    (signing_key.clone(), *signing_key.verifying_key())
}

fn real_ethereum_signature(account_id: u64) -> (EthereumSignature, [u8; 33]) {
    let (_, verifying_key, signature) = create_signed_message_and_keypair(account_id);
    (signature, verifying_key)
}

fn real_attestation(account_id: u64) -> Attestation {
    let (signature, verifying_key) = real_ethereum_signature(account_id);
    Attestation::EthereumAttestation {
        signature,
        proposed_pub_key: verifying_key,
        state_root: H256::zero(),
    }
}

fn real_attestation_key(account_id: u64) -> AttestationKey {
    let (_, verifying_key, _) = create_signed_message_and_keypair(account_id);
    AttestationKey::EthereumKey {
        pub_key: verifying_key,
    }
}

#[test]
fn register_attestation_key_success() {
    new_test_ext().execute_with(|| {
        let account_id: u64 = 1;
        let (_, public_key, signature) = create_signed_message_and_keypair(account_id);

        let registration = RegisterExternalAddress::EthereumAddress {
            signature,
            proposed_pub_key: public_key,
        };

        assert_ok!(Pallet::<Test>::register_attestation_key(
            RuntimeOrigin::root(),
            account_id,
            registration
        ));

        let keys = Pallet::<Test>::validators();
        assert!(keys.iter().any(|(id, attestation_key)| *id == account_id
            && *attestation_key
                == sxt_core::attestation::AttestationKey::EthereumKey {
                    pub_key: public_key
                }));
    });
}

#[test]
fn attest_block_success() {
    new_test_ext().execute_with(|| {
        System::set_block_number(15);
        let account_id: u64 = 1;
        let block_number: u32 = 10;

        // Generate a keypair and create a signed message.
        let (_, public_key, signature) = create_signed_message_and_keypair(account_id);

        // Register the attestation key for the account.
        let registration = RegisterExternalAddress::EthereumAddress {
            signature,
            proposed_pub_key: public_key,
        };
        assert_ok!(Pallet::<Test>::register_attestation_key(
            RuntimeOrigin::root(),
            account_id,
            registration
        ));

        // Create an attestation using the same signature and public key.
        let attestation = Attestation::EthereumAttestation {
            signature,
            proposed_pub_key: public_key,
            state_root: H256::zero(),
        };

        // Submit the attestation.
        assert_ok!(Pallet::<Test>::attest_block(
            RuntimeOrigin::signed(account_id),
            block_number,
            attestation
        ));

        // Verify that the attestation is stored correctly in the pallet's storage.
        let attestations = Pallet::<Test>::attestations(block_number);
        assert!(attestations
            .iter()
            .any(|stored_attestation| *stored_attestation == attestation));
    });
}

#[test]
fn register_attestation_key_fails_if_key_already_registered() {
    new_test_ext().execute_with(|| {
        let account_id_1: u64 = 1;
        let account_id_2: u64 = 2;

        // Generate a keypair and create a signed message.
        let (_, public_key, signature) = create_signed_message_and_keypair(account_id_1);

        // Register the key for the first account.
        let registration = RegisterExternalAddress::EthereumAddress {
            signature,
            proposed_pub_key: public_key,
        };
        assert_ok!(Pallet::<Test>::register_attestation_key(
            RuntimeOrigin::root(),
            account_id_1,
            registration
        ));

        // Attempt to register the same key for a different account.
        let duplicate_registration = RegisterExternalAddress::EthereumAddress {
            signature,
            proposed_pub_key: public_key,
        };
        assert_err!(
            Pallet::<Test>::register_attestation_key(
                RuntimeOrigin::root(),
                account_id_2,
                duplicate_registration
            ),
            Error::<Test>::PublicKeyAlreadyRegistered
        );
    });
}

#[test]
fn register_attestation_key_fails_if_account_already_registered() {
    new_test_ext().execute_with(|| {
        let account_id: u64 = 1;

        // Generate the first keypair and register it.
        let (_, public_key_1, signature_1) = create_signed_message_and_keypair(account_id);
        let registration_1 = RegisterExternalAddress::EthereumAddress {
            signature: signature_1,
            proposed_pub_key: public_key_1,
        };
        assert_ok!(Pallet::<Test>::register_attestation_key(
            RuntimeOrigin::root(),
            account_id,
            registration_1
        ));

        // Generate a second keypair and attempt to register it for the same account.
        let (_, public_key_2, signature_2) = create_signed_message_and_keypair(account_id);
        let registration_2 = RegisterExternalAddress::EthereumAddress {
            signature: signature_2,
            proposed_pub_key: public_key_2,
        };
        assert_err!(
            Pallet::<Test>::register_attestation_key(
                RuntimeOrigin::root(),
                account_id,
                registration_2
            ),
            Error::<Test>::AccountIdAlreadyLinked
        );
    });
}

#[test]
fn register_attestation_key_fails_if_signature_invalid() {
    new_test_ext().execute_with(|| {
        let account_id: u64 = 1;

        // Generate a keypair and create a signed message for a different account.
        let (_, public_key, _) = create_signed_message_and_keypair(999); // Different account ID

        // Attempt to register with an invalid signature.
        let invalid_signature = EthereumSignature {
            r: [0u8; 32],
            s: [0u8; 32],
            v: 27,
        };
        let registration = RegisterExternalAddress::EthereumAddress {
            signature: invalid_signature,
            proposed_pub_key: public_key,
        };
        assert_err!(
            Pallet::<Test>::register_attestation_key(
                RuntimeOrigin::root(),
                account_id,
                registration
            ),
            Error::<Test>::VerificationError
        );
    });
}

#[test]
fn attest_block_fails_if_account_not_registered() {
    new_test_ext().execute_with(|| {
        System::set_block_number(15);
        let account_id: u64 = 1;
        let block_number: u32 = 10;

        // Generate a keypair and create an attestation.
        let (_, public_key, signature) = create_signed_message_and_keypair(account_id);
        let attestation = Attestation::EthereumAttestation {
            signature,
            proposed_pub_key: public_key,
            state_root: H256::zero(),
        };

        // Attempt to attest without registering the account.
        assert_err!(
            Pallet::<Test>::attest_block(
                RuntimeOrigin::signed(account_id),
                block_number,
                attestation
            ),
            Error::<Test>::InsufficientPermissions
        );
    });
}

#[test]
fn attest_block_fails_if_duplicate_attestation() {
    new_test_ext().execute_with(|| {
        System::set_block_number(15);
        let account_id: u64 = 1;
        let block_number: u32 = 10;

        // Generate a keypair and create an attestation.
        let (_, public_key, signature) = create_signed_message_and_keypair(account_id);
        let attestation = Attestation::EthereumAttestation {
            signature,
            proposed_pub_key: public_key,
            state_root: H256::zero(),
        };

        // Register the attestation key.
        let registration = RegisterExternalAddress::EthereumAddress {
            signature,
            proposed_pub_key: public_key,
        };
        assert_ok!(Pallet::<Test>::register_attestation_key(
            RuntimeOrigin::root(),
            account_id,
            registration
        ));

        // Submit the attestation.
        assert_ok!(Pallet::<Test>::attest_block(
            RuntimeOrigin::signed(account_id),
            block_number,
            attestation
        ));

        // Attempt to submit the same attestation again.
        assert_err!(
            Pallet::<Test>::attest_block(
                RuntimeOrigin::signed(account_id),
                block_number,
                attestation
            ),
            Error::<Test>::AttestationAlreadyRecordedError
        );
    });
}

#[test]
fn attest_block_fails_if_future_block() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let account_id: u64 = 1;
        let future_block_number: u32 = 1000;

        // Generate a keypair and create an attestation.
        let (_, public_key, signature) = create_signed_message_and_keypair(account_id);
        let attestation = Attestation::EthereumAttestation {
            signature,
            proposed_pub_key: public_key,
            state_root: H256::zero(),
        };

        // Register the attestation key.
        let registration = RegisterExternalAddress::EthereumAddress {
            signature,
            proposed_pub_key: public_key,
        };
        assert_ok!(Pallet::<Test>::register_attestation_key(
            RuntimeOrigin::root(),
            account_id,
            registration
        ));

        // Attempt to attest a future block.
        assert_err!(
            Pallet::<Test>::attest_block(
                RuntimeOrigin::signed(account_id),
                future_block_number,
                attestation
            ),
            Error::<Test>::CannotAttestFutureBlock
        );
    });
}

#[test]
fn remove_attestation_key_success() {
    new_test_ext().execute_with(|| {
        let account_id: u64 = 1;
        let (_, public_key, signature) = create_signed_message_and_keypair(account_id);

        let registration = RegisterExternalAddress::EthereumAddress {
            signature,
            proposed_pub_key: public_key,
        };

        // Register the attestation key first
        assert_ok!(Pallet::<Test>::register_attestation_key(
            RuntimeOrigin::root(),
            account_id,
            registration.clone()
        ));

        let keys = Pallet::<Test>::validators();
        assert!(keys.iter().any(|(id, attestation_key)| *id == account_id
            && *attestation_key
                == sxt_core::attestation::AttestationKey::EthereumKey {
                    pub_key: public_key
                }));

        // Now remove the key
        assert_ok!(Pallet::<Test>::remove_attestation_key(
            RuntimeOrigin::root(),
            account_id,
            sxt_core::attestation::AttestationKey::EthereumKey {
                pub_key: public_key
            }
        ));

        // Verify the key is no longer in storage
        let keys = Pallet::<Test>::validators();
        assert!(!keys.iter().any(|(id, attestation_key)| *id == account_id
            && *attestation_key
                == sxt_core::attestation::AttestationKey::EthereumKey {
                    pub_key: public_key
                }));
    });
}

#[test]
fn remove_attestation_key_not_found() {
    new_test_ext().execute_with(|| {
        let account_id: u64 = 1;
        let (_, public_key, _) = create_signed_message_and_keypair(account_id);

        // Attempt to remove a key that hasn't been registered
        assert_noop!(
            Pallet::<Test>::remove_attestation_key(
                RuntimeOrigin::root(),
                account_id,
                sxt_core::attestation::AttestationKey::EthereumKey {
                    pub_key: public_key
                }
            ),
            Error::<Test>::KeyNotFound
        );
    });
}
