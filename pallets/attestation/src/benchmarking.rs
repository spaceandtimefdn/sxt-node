//! Benchmarking setup for pallet-attestation
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;

use super::*;
#[allow(unused)]
use crate::Pallet as AttestationPallet;

#[benchmarks]
mod benchmarks {
    use alloc::vec::Vec;

    use codec::Encode;
    use frame_support::{assert_ok, BoundedVec};
    use k256::ecdsa::SigningKey;
    use pallet_keystore::Pallet as Keystore;
    use pallet_permissions::Pallet as Permissions;
    use sha3::digest::generic_array::GenericArray;
    use sxt_core::attestation::{
        blake2_256, sign_eth_message, Attestation, AttestationKey, EthereumSignature,
        RegisterExternalAddress,
    };
    use sxt_core::permissions::{AttestationPalletPermission, PermissionLevel, PermissionList};

    use super::*;
    // Deterministic key generation using `blake2_256`
    fn create_signed_message_and_keypair(
        message: Vec<u8>,
        block_number: Option<BlockNumber>,
    ) -> (SigningKey, [u8; 33], EthereumSignature) {
        // Convert the seed to bytes and hash it to generate a 32-byte private key
        let private_key_bytes = blake2_256(&message);

        let signing_key = SigningKey::from_bytes(GenericArray::from_slice(&private_key_bytes))
            .expect("Valid private key");
        let verifying_key = signing_key.verifying_key();
        let verifying_key_sec1 = &*verifying_key.to_sec1_bytes();
        let verifying_key_sec1: [u8; 33] = verifying_key_sec1.try_into().unwrap();

        // Sign the message (account ID encoded as bytes)
        let message = message
            .into_iter()
            .chain(
                block_number
                    .map(|block_number| (block_number as u64).to_be_bytes().to_vec())
                    .unwrap_or_default(),
            )
            .collect::<alloc::vec::Vec<_>>();
        let signature =
            sign_eth_message(&private_key_bytes, &message).expect("Failed to sign message");

        (signing_key, verifying_key_sec1, signature)
    }

    // Helper function to create a registered attestation key
    fn create_registered_attestation_key<T: Config>(account_id: T::AccountId) -> AttestationKey {
        let (_, public_key, signature) =
            create_signed_message_and_keypair(account_id.encode(), None);
        let address20 =
            sxt_core::attestation::uncompressed_public_key_to_address(&public_key).unwrap();
        let registration = RegisterExternalAddress::EthereumAddress {
            signature,
            proposed_pub_key: public_key,
            address20: address20.clone(),
        };

        assert_ok!(Keystore::<T>::register_key(
            RawOrigin::Root.into(),
            account_id.clone(),
            registration
        ));

        let permissions =
            PermissionList::try_from(alloc::vec![PermissionLevel::AttestationPallet(
                AttestationPalletPermission::AttestBlock,
            )])
            .unwrap();

        assert_ok!(Permissions::<T>::set_permissions(
            RawOrigin::Root.into(),
            account_id.clone(),
            permissions
        ));

        AttestationKey::EthereumKey {
            pub_key: public_key,
            address20,
        }
    }

    #[benchmark]
    fn attest_block() {
        let current_block: u32 = 15;
        frame_system::Pallet::<T>::set_block_number(current_block.into());

        let caller: T::AccountId = whitelisted_caller();
        let block_number: BlockNumber = 10;

        // Register the attestation key
        let attestation_key = create_registered_attestation_key::<T>(caller.clone());

        // Generate deterministic attestation
        let (_, public_key, signature) =
            create_signed_message_and_keypair(caller.encode(), Some(block_number));

        let address20 =
            sxt_core::attestation::uncompressed_public_key_to_address(&public_key).unwrap();

        let block_hash = T::Hash::default();
        let attestation = Attestation::EthereumAttestation {
            signature,
            proposed_pub_key: public_key,
            state_root: caller.encode().try_into().unwrap(),
            address20,
            block_number,
            block_hash,
        };

        #[extrinsic_call]
        attest_block(
            RawOrigin::Signed(caller.clone()),
            Some(block_number),
            attestation.clone(),
        );

        // Assert that the attestation was recorded
        let attestations = Attestations::<T>::get(block_number);
        assert!(attestations.iter().any(|stored| stored == &attestation));
    }

    #[benchmark]
    fn mark_block_forwarded() {
        let caller: T::AccountId = whitelisted_caller();

        pallet_permissions::Pallet::<T>::add_proxy_permission(
            RawOrigin::Root.into(),
            caller.clone(),
            PermissionLevel::AttestationPallet(AttestationPalletPermission::ForwardAttestedBlock),
        )
        .unwrap();

        assert_eq!(LastForwardedBlock::<T>::get(), None);

        #[extrinsic_call]
        mark_block_forwarded(RawOrigin::Signed(caller), 1);

        assert_eq!(LastForwardedBlock::<T>::get(), Some(1));
    }

    impl_benchmark_test_suite!(
        AttestationPallet,
        crate::mock::new_test_ext(),
        crate::mock::Test
    );
}
