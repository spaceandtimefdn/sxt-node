use alloy_primitives::Address;
use codec::{Decode, Encode, MaxEncodedLen};
use k256::ecdsa::VerifyingKey;
use polkadot_sdk::sp_core::{ecdsa, RuntimeDebug};
use polkadot_sdk::sp_runtime;
use polkadot_sdk::sp_runtime::traits::{IdentifyAccount, Lazy, Verify};
use polkadot_sdk::sp_runtime::AccountId32;
use scale_info::TypeInfo;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

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
        (*Address::from_public_key(
            &VerifyingKey::from_sec1_bytes(&self.0)
                .expect("ecdsa::Public key should be in compressed form"),
        )
        .into_word())
        .into()
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

        &AccountId32::from(*address.into_word()) == signer
    }
}

#[cfg(test)]
mod tests {
    use alloy_signer::SignerSync;
    use alloy_signer_local::PrivateKeySigner;

    use super::*;

    #[test]
    fn we_can_identify_account_id() {
        let signer = PrivateKeySigner::random();

        let verifying_key = signer.credential().verifying_key();

        let eth_ecdsa_signer =
            EthEcdsaSigner(ecdsa::Public::try_from(&verifying_key.to_sec1_bytes()[..]).unwrap());

        let address = signer.address();

        let expected_account_id = AccountId32::new(
            core::iter::repeat_n(0, 12)
                .chain(address.into_array())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        );

        assert_eq!(eth_ecdsa_signer.into_account(), expected_account_id)
    }

    fn valid_verification_input() -> (EthEcdsaSignature, Vec<u8>, AccountId32) {
        let signer = PrivateKeySigner::random();
        let message = b"Hello World!";

        let address = signer.address();

        let account_id = AccountId32::new(
            core::iter::repeat_n(0, 12)
                .chain(address.into_array())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        );

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
