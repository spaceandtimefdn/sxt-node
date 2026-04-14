use codec::{Decode, Encode, MaxEncodedLen};
use polkadot_sdk::sp_core::{ecdsa, ed25519, sr25519, RuntimeDebug};
use polkadot_sdk::sp_runtime;
use polkadot_sdk::sp_runtime::traits::{IdentifyAccount, Lazy, Verify};
use polkadot_sdk::sp_runtime::AccountId32;
use scale_info::TypeInfo;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use snafu::Snafu;

use crate::{EthEcdsaSignature, EthEcdsaSigner};

/// Signature verify that can work with any known signature types.
///
/// Implemented similarly to `sp_runtime::MultiSignature` but with an extra variant `EthEcdsa`.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Eq, PartialEq, Clone, Encode, Decode, MaxEncodedLen, RuntimeDebug, TypeInfo)]
pub enum MultiSignature {
    /// An Ed25519 signature.
    Ed25519(ed25519::Signature),
    /// An Sr25519 signature.
    Sr25519(sr25519::Signature),
    /// An ECDSA/SECP256k1 signature.
    Ecdsa(ecdsa::Signature),
    /// An Eth ecdsa signature of EIP-191 data.
    EthEcdsa(EthEcdsaSignature),
}

/// Public key for any known crypto algorithm.
///
/// Implemented similarly to `sp_runtime::MultiSignature` but with an extra variant `EthEcdsa`.
#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum MultiSigner {
    /// An Ed25519 identity.
    Ed25519(ed25519::Public),
    /// An Sr25519 identity.
    Sr25519(sr25519::Public),
    /// An SECP256k1/ECDSA identity (actually, the Blake2 hash of the compressed pub key).
    Ecdsa(ecdsa::Public),
    /// An Ethereum SECP256k1/ECDSA identity, with the ethereum account id prepended by 12 0-bytes.
    EthEcdsa(EthEcdsaSigner),
}

/// Encountered EthEcdsa variant unexpectedly.
#[derive(Debug, Snafu)]
#[snafu(display("encountered EthEcdsa variant unexpectedly"))]
pub struct EncounteredEthEcdsa;

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
    type Error = EncounteredEthEcdsa;

    fn try_from(value: MultiSignature) -> Result<Self, Self::Error> {
        match value {
            MultiSignature::Ed25519(signature) => {
                Ok(sp_runtime::MultiSignature::Ed25519(signature))
            }
            MultiSignature::Sr25519(signature) => {
                Ok(sp_runtime::MultiSignature::Sr25519(signature))
            }
            MultiSignature::Ecdsa(signature) => Ok(sp_runtime::MultiSignature::Ecdsa(signature)),
            MultiSignature::EthEcdsa(_) => Err(EncounteredEthEcdsa),
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
    type Error = EncounteredEthEcdsa;

    fn try_from(value: MultiSigner) -> Result<Self, Self::Error> {
        match value {
            MultiSigner::Ed25519(signature) => Ok(sp_runtime::MultiSigner::Ed25519(signature)),
            MultiSigner::Sr25519(signature) => Ok(sp_runtime::MultiSigner::Sr25519(signature)),
            MultiSigner::Ecdsa(signature) => Ok(sp_runtime::MultiSigner::Ecdsa(signature)),
            MultiSigner::EthEcdsa(_) => Err(EncounteredEthEcdsa),
        }
    }
}

impl IdentifyAccount for MultiSigner {
    type AccountId = AccountId32;
    fn into_account(self) -> AccountId32 {
        match self {
            MultiSigner::EthEcdsa(eth_ecdsa_signer) => eth_ecdsa_signer.into_account(),
            sp_signer => sp_runtime::MultiSigner::try_from(sp_signer)
                .expect("this is not eth-ecdsa")
                .into_account(),
        }
    }
}

impl Verify for MultiSignature {
    type Signer = MultiSigner;
    fn verify<L: Lazy<[u8]>>(&self, msg: L, signer: &AccountId32) -> bool {
        match self {
            Self::EthEcdsa(eth_ecdsa_sig) => eth_ecdsa_sig.verify(msg, signer),
            sp_signature => sp_runtime::MultiSignature::try_from(sp_signature.clone())
                .expect("this is not eth-ecdsa")
                .verify(msg, signer),
        }
    }
}

impl From<ed25519::Signature> for MultiSignature {
    fn from(x: ed25519::Signature) -> Self {
        sp_runtime::MultiSignature::from(x).into()
    }
}

impl From<ed25519::Public> for MultiSigner {
    fn from(x: ed25519::Public) -> Self {
        sp_runtime::MultiSigner::from(x).into()
    }
}

impl From<sr25519::Signature> for MultiSignature {
    fn from(x: sr25519::Signature) -> Self {
        sp_runtime::MultiSignature::from(x).into()
    }
}

impl TryFrom<MultiSignature> for sr25519::Signature {
    type Error = ();

    fn try_from(value: MultiSignature) -> Result<Self, Self::Error> {
        sp_runtime::MultiSignature::try_from(value)
            .map_err(|_| ())?
            .try_into()
    }
}

impl From<sr25519::Public> for MultiSigner {
    fn from(x: sr25519::Public) -> Self {
        sp_runtime::MultiSigner::from(x).into()
    }
}

impl TryFrom<MultiSigner> for sr25519::Public {
    type Error = ();

    fn try_from(value: MultiSigner) -> Result<Self, Self::Error> {
        sp_runtime::MultiSigner::try_from(value)
            .map_err(|_| ())?
            .try_into()
    }
}

impl From<ecdsa::Signature> for MultiSignature {
    fn from(x: ecdsa::Signature) -> Self {
        sp_runtime::MultiSignature::from(x).into()
    }
}

impl From<ecdsa::Public> for MultiSigner {
    fn from(x: ecdsa::Public) -> Self {
        sp_runtime::MultiSigner::from(x).into()
    }
}

impl From<EthEcdsaSignature> for MultiSignature {
    fn from(x: EthEcdsaSignature) -> Self {
        MultiSignature::EthEcdsa(x)
    }
}

impl From<EthEcdsaSigner> for MultiSigner {
    fn from(x: EthEcdsaSigner) -> Self {
        MultiSigner::EthEcdsa(x)
    }
}
