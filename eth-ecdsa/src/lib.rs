#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]

mod signature;
pub use signature::{EthEcdsaSignature, EthEcdsaSigner};

mod multi_signature;
pub use multi_signature::MultiSignature;
