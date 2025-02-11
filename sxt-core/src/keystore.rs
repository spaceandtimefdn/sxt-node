//! types for pallet keystore
use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
pub use sp_core::hashing::{blake2_128, blake2_256};
pub use sp_core::{RuntimeDebug, H256};

use crate::attestation::Address20;

/// A struct for holding offchain keys that an account id is associated with
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default)]
pub struct UserKeystore {
    /// Ethereum style ECDSA key
    pub eth_key: Option<EthereumKey>,
}

impl UserKeystore {
    /// Construct a UserKeystore from an existing ethereum key
    /// Needless update is needed while UserKeystore has only 1 type
    /// we want to preserve the spread syntax for later use
    #[allow(clippy::needless_update)]
    pub fn with_eth_key(self, eth_key: Option<EthereumKey>) -> Self {
        Self { eth_key, ..self }
    }
}

/// Types of keys that can be stored on chain
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum KeyType {
    /// An Ethereum keystore type
    Ethereum(EthereumKey),
}

/// A representation of an ethereum public key that can be stored on chain
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct EthereumKey {
    /// The public key in sec1 bytes
    pub pub_key: [u8; 33],
    /// Ethereum style 20 byte address
    pub address20: Address20,
}

/// The types of addresses that can be unregistered
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum UnregisterExternalAddress {
    /// Unregistration message for an ethereum address
    EthereumAddress,
}
