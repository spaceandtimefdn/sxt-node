#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(feature = "std")]
mod null_bytes;
#[cfg(feature = "std")]
pub use null_bytes::column_remove_null_bytes;

mod nulls;
pub use nulls::column_def_not_null;
#[cfg(feature = "std")]
pub use nulls::column_default_nulls;

mod decimal_precision;
#[cfg(feature = "std")]
pub use decimal_precision::column_clamp_precision;
pub use decimal_precision::column_def_clamp_precision;

#[cfg(feature = "std")]
mod parse_decimals;
#[cfg(feature = "std")]
pub use parse_decimals::{
    column_parse_decimals_fallible,
    column_parse_decimals_unchecked,
    ParseDecimalsError,
};

#[cfg(feature = "std")]
mod record_batch_map;
#[cfg(feature = "std")]
pub use record_batch_map::{
    record_batch_map,
    record_batch_map_with_target_types,
    record_batch_try_map_with_target_types,
    MapOrTargetTypeError,
    TargetTypeNotFound,
};
