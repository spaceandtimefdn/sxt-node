use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use snafu::Snafu;
use sp_core::{ecdsa, ed25519, sr25519, RuntimeDebug};
use sp_runtime::traits::{IdentifyAccount, Lazy, Verify};
use sp_runtime::AccountId32;

/// Signature verify that can work with any known signature types.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Eq, PartialEq, Clone, Encode, Decode, MaxEncodedLen, RuntimeDebug, TypeInfo)]
pub enum MultiSignature {
    /// An Ed25519 signature.
    Ed25519(ed25519::Signature),
    /// An Sr25519 signature.
    Sr25519(sr25519::Signature),
    /// An ECDSA/SECP256k1 signature.
    Ecdsa(ecdsa::Signature),
    /// An Eth signature.
    EthEcdsa(ecdsa::Signature),
}

/// Public key for any known crypto algorithm.
#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum MultiSigner {
    /// An Ed25519 identity.
    Ed25519(ed25519::Public),
    /// An Sr25519 identity.
    Sr25519(sr25519::Public),
    /// An SECP256k1/ECDSA identity (actually, the Blake2 hash of the compressed pub key).
    Ecdsa(ecdsa::Public),
    /// An Eth address?
    EthEcdsa(ecdsa::Public),
}

#[derive(Debug, Snafu)]
#[snafu(display("encountered EthEcdsa variant unexpectedly"))]
pub struct EthEcdsa;

impl From<sp_runtime::MultiSignature> for MultiSignature {
    fn from(value: sp_runtime::MultiSignature) -> Self {
        match value {
            sp_runtime::MultiSignature::Ed25519(signature) => MultiSignature::Ed25519(signature),
            sp_runtime::MultiSignature::Sr25519(signature) => MultiSignature::Sr25519(signature),
            sp_runtime::MultiSignature::Ecdsa(signature) => MultiSignature::Ecdsa(signature),
        }
    }
}

impl TryFrom<MultiSignature> for sp_runtime::MultiSignature {
    type Error = EthEcdsa;

    fn try_from(value: MultiSignature) -> Result<Self, Self::Error> {
        match value {
            MultiSignature::Ed25519(signature) => {
                Ok(sp_runtime::MultiSignature::Ed25519(signature))
            }
            MultiSignature::Sr25519(signature) => {
                Ok(sp_runtime::MultiSignature::Sr25519(signature))
            }
            MultiSignature::Ecdsa(signature) => Ok(sp_runtime::MultiSignature::Ecdsa(signature)),
            MultiSignature::EthEcdsa(_) => Err(EthEcdsa),
        }
    }
}

impl From<sp_runtime::MultiSigner> for MultiSigner {
    fn from(value: sp_runtime::MultiSigner) -> Self {
        match value {
            sp_runtime::MultiSigner::Ed25519(signature) => MultiSigner::Ed25519(signature),
            sp_runtime::MultiSigner::Sr25519(signature) => MultiSigner::Sr25519(signature),
            sp_runtime::MultiSigner::Ecdsa(signature) => MultiSigner::Ecdsa(signature),
        }
    }
}

impl TryFrom<MultiSigner> for sp_runtime::MultiSigner {
    type Error = EthEcdsa;

    fn try_from(value: MultiSigner) -> Result<Self, Self::Error> {
        match value {
            MultiSigner::Ed25519(signature) => Ok(sp_runtime::MultiSigner::Ed25519(signature)),
            MultiSigner::Sr25519(signature) => Ok(sp_runtime::MultiSigner::Sr25519(signature)),
            MultiSigner::Ecdsa(signature) => Ok(sp_runtime::MultiSigner::Ecdsa(signature)),
            MultiSigner::EthEcdsa(_) => Err(EthEcdsa),
        }
    }
}

impl IdentifyAccount for MultiSigner {
    type AccountId = AccountId32;
    fn into_account(self) -> AccountId32 {
        match self {
            Self::EthEcdsa(who) => sp_io::hashing::blake2_256(who.as_ref()).into(),
            sp_signer => sp_runtime::MultiSigner::try_from(sp_signer)
                .expect("this is not eth-ecdsa")
                .into_account(),
        }
    }
}

impl Verify for MultiSignature {
    type Signer = MultiSigner;
    fn verify<L: Lazy<[u8]>>(&self, mut msg: L, signer: &AccountId32) -> bool {
        let who: [u8; 32] = *signer.as_ref();
        match self {
            Self::EthEcdsa(sig) => {
                let m = sp_io::hashing::blake2_256(msg.get());
                sp_io::crypto::secp256k1_ecdsa_recover_compressed(sig.as_ref(), &m)
                    .map_or(false, |pubkey| sp_io::hashing::blake2_256(&pubkey) == who)
            }
            sp_signature => sp_runtime::MultiSignature::try_from(sp_signature.clone())
                .expect("this is not eth-ecdsa")
                .verify(msg, signer),
        }
    }
}

impl From<sr25519::Signature> for MultiSignature {
    fn from(x: sr25519::Signature) -> Self {
        sp_runtime::MultiSignature::from(x).into()
    }
}

impl From<sr25519::Public> for MultiSigner {
    fn from(x: sr25519::Public) -> Self {
        sp_runtime::MultiSigner::from(x).into()
    }
}
