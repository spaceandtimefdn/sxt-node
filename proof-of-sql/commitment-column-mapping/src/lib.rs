#![doc = include_str!("../README.md")]
#![no_std]

extern crate alloc;

mod map_column_commitment_metadata;

mod try_map_on_chain_column;

mod combinator;

mod map_table_commitment;

mod map_on_chain_table;

mod varchar_workaround;
pub use varchar_workaround::{
    convert_selected_varbinary_columns_to_varchar,
    convert_varchar_to_varbinary,
    ConvertSelectedVarbinaryColumnsToVarcharError,
};
