#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

mod column_options;

mod column_type_conversion;

mod metadata_prefix;

mod row_number_column;
