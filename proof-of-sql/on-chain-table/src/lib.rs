#![cfg_attr(not(feature = "std"), no_std)]
#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

extern crate alloc;

#[cfg(feature = "arrow")]
mod i256_conversion;

mod u256_scalar_conversion;
pub use u256_scalar_conversion::OutOfScalarBounds;

mod map;

mod column;
pub use column::OnChainColumn;

#[cfg(feature = "arrow")]
mod arrow_column_conversion;
#[cfg(feature = "arrow")]
pub use arrow_column_conversion::ArrowToOnChainColumnError;

mod table;
pub use table::{OnChainTable, OnChainTableError};

#[cfg(feature = "arrow")]
mod arrow_table_conversion;
#[cfg(feature = "arrow")]
pub use arrow_table_conversion::ArrowToOnChainTableError;
