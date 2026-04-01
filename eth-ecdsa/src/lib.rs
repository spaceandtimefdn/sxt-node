#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "serde", warn(unused_crate_dependencies))]

/// Ethereum-compatible ECDSA signature implementation.
mod signature;
pub use signature::{EthEcdsaSignature, EthEcdsaSigner};

/// Multi-signature support for combining multiple signature types.
mod multi_signature;
pub use multi_signature::MultiSignature;
