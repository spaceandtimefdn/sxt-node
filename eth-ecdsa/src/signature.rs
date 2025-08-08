use alloy_primitives::Address;
use codec::{Decode, Encode, MaxEncodedLen};
use k256::ecdsa::VerifyingKey;
use scale_info::TypeInfo;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use sp_core::{ecdsa, RuntimeDebug};
use sp_runtime::traits::{IdentifyAccount, Lazy, Verify};
use sp_runtime::AccountId32;

/// Wrapper type over an ECDSA signature (a 512-bit value, plus 8 bits for recovery ID).
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Eq, PartialEq, Clone, Encode, Decode, MaxEncodedLen, RuntimeDebug, TypeInfo)]
pub struct EthEcdsaSignature(pub ecdsa::Signature);

/// Wrapper type over an ECDSA compressed public key.
#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct EthEcdsaSigner(pub ecdsa::Public);

impl IdentifyAccount for EthEcdsaSigner {
    type AccountId = AccountId32;
    fn into_account(self) -> AccountId32 {
        AccountId32::new(sp_io::hashing::blake2_256(
            Address::from_public_key(
                &VerifyingKey::from_sec1_bytes(&self.0)
                    .expect("ecdsa::Public key should be in compressed form"),
            )
            .as_slice(),
        ))
    }
}

impl Verify for EthEcdsaSignature {
    type Signer = EthEcdsaSigner;
    fn verify<L: Lazy<[u8]>>(&self, mut msg: L, signer: &AccountId32) -> bool {
        let Ok(alloy_sig) = alloy_primitives::PrimitiveSignature::from_raw_array(&self.0 .0) else {
            return false;
        };

        let Ok(address) = alloy_sig.recover_address_from_msg(msg.get()) else {
            return false;
        };

        &sp_io::hashing::blake2_256(address.as_ref()) == <dyn AsRef<[u8; 32]>>::as_ref(signer)
    }
}

#[cfg(test)]
mod tests {
    use alloy_signer::SignerSync;
    use alloy_signer_local::PrivateKeySigner;
    use sp_core::crypto::Ss58Codec;

    use super::*;

    #[test]
    fn our_account_ids_match_polkagate_account_id() {
        let test_ss58 = "5CNWk7dqqxdEY5aMn1JBPpwUrh7nCCE3JxZXr3KcsJv8Hr1Z";
        let test_seed = "f1e0c57b0c85d60c2086ff468831fabd13b22530aa8b46aa696295197ddcab43";
        let _test_recovery_phrase =
            "priority dwarf mixed bike approve double vacuum village project slow moral large";

        let signer: PrivateKeySigner = test_seed.parse().unwrap();

        let verifying_key = signer.credential().verifying_key();

        let eth_ecdsa_signer =
            EthEcdsaSigner(ecdsa::Public::try_from(&verifying_key.to_sec1_bytes()[..]).unwrap());

        let account = eth_ecdsa_signer.into_account();

        assert_eq!(test_ss58, Ss58Codec::to_ss58check(&account));
    }

    #[test]
    fn we_can_identify_account_id() {
        let signer = PrivateKeySigner::random();

        let verifying_key = signer.credential().verifying_key();

        let eth_ecdsa_signer =
            EthEcdsaSigner(ecdsa::Public::try_from(&verifying_key.to_sec1_bytes()[..]).unwrap());

        let address = signer.address();

        let expected_account_id = AccountId32::new(sp_io::hashing::blake2_256(address.as_ref()));

        assert_eq!(eth_ecdsa_signer.into_account(), expected_account_id)
    }

    fn valid_verification_input() -> (EthEcdsaSignature, Vec<u8>, AccountId32) {
        let signer = PrivateKeySigner::random();
        let message = b"Hello World!";

        let address = signer.address();

        let account_id = AccountId32::new(sp_io::hashing::blake2_256(address.as_ref()));

        let signature = signer.sign_message_sync(message).unwrap();

        let eth_ecdsa_signature = EthEcdsaSignature(signature.as_bytes().into());

        (eth_ecdsa_signature, message.to_vec(), account_id)
    }

    #[test]
    fn we_can_verify_signature() {
        let (eth_ecdsa_signature, message, account_id) = valid_verification_input();

        assert!(eth_ecdsa_signature.verify(&message[..], &account_id));
    }

    #[test]
    fn we_cannot_verify_signature_with_wrong_message() {
        let (eth_ecdsa_signature, _, account_id) = valid_verification_input();

        assert!(!eth_ecdsa_signature.verify(&b"Goodbye."[..], &account_id));
    }

    #[test]
    fn we_cannot_verify_signature_with_wrong_account() {
        let (eth_ecdsa_signature, message, mut account_id) = valid_verification_input();

        let account_id_array = <sp_runtime::AccountId32 as AsMut<[u8]>>::as_mut(&mut account_id);
        account_id_array[0] = account_id_array[0].wrapping_add(1);
        assert!(!eth_ecdsa_signature.verify(&message[..], &account_id));
    }
}
