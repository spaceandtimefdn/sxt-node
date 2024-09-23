#![cfg_attr(not(feature = "std"), no_std)]
#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

extern crate alloc;

#[cfg(feature = "arrow")]
mod i256_conversion;

mod column;
pub use column::OnChainColumn;

#[cfg(feature = "arrow")]
mod arrow_column_conversion;
#[cfg(feature = "arrow")]
pub use arrow_column_conversion::ArrowToOnChainColumnError;
