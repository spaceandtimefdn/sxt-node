use codec::Encode;
use frame_support::{assert_err, assert_ok};
use k256::ecdsa::{SigningKey, VerifyingKey};
use sp_core::H256;
use sp_runtime::BoundedVec;
use sxt_core::attestation::{
    create_attestation_message,
    sign_eth_message,
    Attestation,
    EthereumSignature,
    RegisterExternalAddress,
};
use sxt_core::permissions::{AttestationPalletPermission, PermissionLevel, PermissionList};

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

#[test]
fn attest_block_success() {
    new_test_ext().execute_with(|| {
        System::set_block_number(15);
        let account_id: u64 = 1;
        let block_number: u32 = 10;

        // Generate a keypair and create a signed message.
        let (private_key, public_key, signature) = create_signed_message_and_keypair(account_id);

        // Compute the address20 from the public key.
        let address20 =
            sxt_core::attestation::uncompressed_public_key_to_address(&public_key).unwrap();

        // Register the attestation key for the account.
        let registration = RegisterExternalAddress::EthereumAddress {
            signature,
            proposed_pub_key: public_key,
            address20: address20.clone(),
        };

        assert_ok!(Keystore::register_key(
            RuntimeOrigin::root(),
            account_id,
            registration
        ));

        let permissions = PermissionList::try_from(vec![PermissionLevel::AttestationPallet(
            AttestationPalletPermission::AttestBlock,
        )])
        .unwrap();
        assert_ok!(Permissions::set_permissions(
            RuntimeOrigin::root(),
            account_id,
            permissions
        ));

        // Create an attestation using the same signature and public key.
        let data: Vec<u8> = vec![0xFF; 64];

        // Convert to BoundedVec<u8, ConstU32<64>>
        let state_root = BoundedVec::try_from(data).expect("Should fit");
        let attestation_message =
            create_attestation_message(state_root.clone().into_inner(), block_number);
        let attestation_signature = sign_eth_message(&private_key.to_bytes(), &attestation_message)
            .expect("could not sign");

        let attestation = Attestation::EthereumAttestation {
            signature: attestation_signature,
            proposed_pub_key: public_key,
            address20,
            state_root,
            block_number,
            block_hash: H256::zero(),
        };

        // Submit the attestation.
        assert_ok!(Pallet::<Test>::attest_block(
            RuntimeOrigin::signed(account_id),
            block_number,
            attestation.clone()
        ));

        // Verify that the attestation is stored correctly in the pallet's storage.
        let attestations = Pallet::<Test>::attestations(block_number);
        assert!(attestations
            .iter()
            .any(|stored_attestation| *stored_attestation == attestation));
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
        let address20 =
            sxt_core::attestation::uncompressed_public_key_to_address(&public_key).unwrap();

        let state_root = BoundedVec::new();
        let attestation = Attestation::EthereumAttestation {
            signature,
            proposed_pub_key: public_key,
            address20,
            state_root,
            block_number,
            block_hash: H256::zero(),
        };

        // Attempt to attest without registering the account.
        assert_err!(
            Pallet::<Test>::attest_block(
                RuntimeOrigin::signed(account_id),
                block_number,
                attestation
            ),
            pallet_permissions::Error::<Test>::InsufficientPermissions
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
        let (private_key, public_key, signature) = create_signed_message_and_keypair(account_id);
        let address20 =
            sxt_core::attestation::uncompressed_public_key_to_address(&public_key).unwrap();

        // Create an attestation using the same signature and public key.
        let data: Vec<u8> = vec![0xFF; 64];

        // Convert to BoundedVec<u8, ConstU32<64>>
        let state_root = BoundedVec::try_from(data).expect("Should fit");
        let attestation_message =
            create_attestation_message(state_root.clone().into_inner(), block_number);
        let attestation_signature = sign_eth_message(&private_key.to_bytes(), &attestation_message)
            .expect("could not sign");

        let attestation = Attestation::EthereumAttestation {
            signature: attestation_signature,
            proposed_pub_key: public_key,
            address20: address20.clone(),
            state_root,
            block_number,
            block_hash: H256::zero(),
        };
        // Register the attestation key.
        let registration = RegisterExternalAddress::EthereumAddress {
            signature,
            proposed_pub_key: public_key,
            address20,
        };
        assert_ok!(Keystore::register_key(
            RuntimeOrigin::root(),
            account_id,
            registration
        ));

        let permissions = PermissionList::try_from(vec![PermissionLevel::AttestationPallet(
            AttestationPalletPermission::AttestBlock,
        )])
        .unwrap();
        assert_ok!(Permissions::set_permissions(
            RuntimeOrigin::root(),
            account_id,
            permissions
        ));

        // Submit the attestation.
        assert_ok!(Pallet::<Test>::attest_block(
            RuntimeOrigin::signed(account_id),
            block_number,
            attestation.clone()
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
        let address20 =
            sxt_core::attestation::uncompressed_public_key_to_address(&public_key).unwrap();

        let attestation = Attestation::EthereumAttestation {
            signature,
            proposed_pub_key: public_key,
            address20: address20.clone(),
            state_root: BoundedVec::new(),
            block_number: future_block_number,
            block_hash: H256::zero(),
        };

        // Register the attestation key.
        let registration = RegisterExternalAddress::EthereumAddress {
            signature,
            proposed_pub_key: public_key,
            address20,
        };
        assert_ok!(Keystore::register_key(
            RuntimeOrigin::root(),
            account_id,
            registration
        ));

        let permissions = PermissionList::try_from(vec![PermissionLevel::AttestationPallet(
            AttestationPalletPermission::AttestBlock,
        )])
        .unwrap();
        assert_ok!(Permissions::set_permissions(
            RuntimeOrigin::root(),
            account_id,
            permissions
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
