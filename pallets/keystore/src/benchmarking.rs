//! Benchmarking setup for pallet-keystore
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;

use super::*;
#[allow(unused)]
use crate::Pallet as KeystorePallet;

#[benchmarks]
mod benchmarks {
    use codec::Encode;
    use frame_support::assert_ok;
    use k256::ecdsa::SigningKey;
    use scale_info::prelude::vec::Vec;
    use sha3::digest::generic_array::GenericArray;
    use sxt_core::attestation::{
        blake2_256,
        sign_eth_message,
        EthereumSignature,
        RegisterExternalAddress,
    };
    use sxt_core::keystore::UnregisterExternalAddress;

    use super::*;

    // Deterministic key generation using `blake2_256`
    fn create_signed_message_and_keypair(
        msg: Vec<u8>,
        seed: u64,
    ) -> (SigningKey, [u8; 33], EthereumSignature) {
        // Convert the seed to bytes and hash it to generate a 32-byte private key
        let seed_bytes = seed.to_le_bytes();
        let private_key_bytes = blake2_256(&seed_bytes);

        let signing_key = SigningKey::from_bytes(GenericArray::from_slice(&private_key_bytes))
            .expect("Valid private key");
        let verifying_key = signing_key.verifying_key();
        let verifying_key_sec1 = &*verifying_key.to_sec1_bytes();
        let verifying_key_sec1: [u8; 33] = verifying_key_sec1.try_into().unwrap();

        // Sign the message (account ID encoded as bytes)
        let signature = sign_eth_message(&private_key_bytes, &msg).expect("Failed to sign message");

        (signing_key, verifying_key_sec1, signature)
    }

    #[benchmark]
    fn register_key() {
        let caller: T::AccountId = whitelisted_caller();
        let caller_encoded: Vec<u8> = caller.encode();
        let seed: u64 = u64::from_le_bytes(caller_encoded[0..8].try_into().unwrap_or([0u8; 8]));

        // Generate a deterministic keypair and signature
        let (private_key, public_key, signature) =
            create_signed_message_and_keypair(caller_encoded, seed);

        let address20 =
            sxt_core::attestation::uncompressed_public_key_to_address(&public_key).unwrap();
        let registration = RegisterExternalAddress::EthereumAddress {
            signature,
            proposed_pub_key: public_key,
            address20,
        };

        #[extrinsic_call]
        register_key(RawOrigin::Root, caller.clone(), registration);

        // Verify that the key has been registered
        assert!(Keys::<T>::contains_key(&caller));
    }

    #[benchmark]
    fn unregister_key() {
        let caller: T::AccountId = whitelisted_caller();
        let caller_encoded = caller.encode();
        let seed: u64 = u64::from_le_bytes(caller_encoded[0..8].try_into().unwrap_or([0u8; 8]));

        // Generate a deterministic keypair and signature
        let (private_key, public_key, signature) =
            create_signed_message_and_keypair(caller_encoded, seed);

        let address20 =
            sxt_core::attestation::uncompressed_public_key_to_address(&public_key).unwrap();
        let registration = RegisterExternalAddress::EthereumAddress {
            signature,
            proposed_pub_key: public_key,
            address20,
        };

        assert_ok!(KeystorePallet::<T>::register_key(
            RawOrigin::Root.into(),
            caller.clone(),
            registration
        ));

        let deregistration = UnregisterExternalAddress::EthereumAddress;
        #[extrinsic_call]
        unregister_key(RawOrigin::Root, caller.clone(), deregistration);

        // Verify the key has been removed
        let keystore = Keys::<T>::get(&caller).unwrap();

        assert!(keystore.eth_key.is_none());
    }

    impl_benchmark_test_suite!(
        KeystorePallet,
        crate::mock::new_test_ext(),
        crate::mock::Test
    );
}
