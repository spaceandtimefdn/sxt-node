#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

mod map;

mod column_options;

mod column_type_conversion;

mod metadata_prefix;

mod row_number_column;

mod validated_create_table;
pub use validated_create_table::{InvalidCreateTable, ValidatedCreateTable};
