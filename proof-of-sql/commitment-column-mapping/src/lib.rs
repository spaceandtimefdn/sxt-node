#![doc = include_str!("../README.md")]
#![no_std]

extern crate alloc;

/// Maps column commitment metadata between representations.
mod map_column_commitment_metadata;

/// Tries to map on-chain columns with error handling.
mod try_map_on_chain_column;

/// Combinator utilities for mapping operations.
mod combinator;

/// Maps table commitments between representations.
mod map_table_commitment;

/// Maps on-chain tables to their commitment representations.
mod map_on_chain_table;

/// Workaround for varchar/varbinary column type handling.
mod varchar_workaround;
pub use varchar_workaround::{
    convert_selected_varbinary_columns_to_varchar,
    convert_varchar_to_varbinary,
    ConvertSelectedVarbinaryColumnsToVarcharError,
};
