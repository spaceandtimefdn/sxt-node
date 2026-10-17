#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(
    all(feature = "proptest", feature = "cpu-perf"),
    warn(unused_crate_dependencies)
)]

extern crate alloc;

/// Internal map types with no_std-compatible hashers.
mod map;

/// Column options parsing and validation.
mod column_options;
pub use column_options::InvalidColumnOptions;

/// Type conversion between SQLParser and proof-of-sql types.
mod column_type_conversion {
    proof_of_sql_unversioned::impl_sqlparser_proof_of_sql_type_conversion!();
}
pub use column_type_conversion::{
    sqlparser_data_type_to_proof_of_sql_column_type,
    UnsupportedColumnType,
};

/// Row number column definition utilities.
mod row_number_column;
pub use row_number_column::row_number_column_def;

/// Create table statement validation.
mod validated_create_table;
pub use validated_create_table::{InvalidCreateTable, ValidatedCreateTable};

/// Create table processing and commitment generation.
mod create_table;
pub use create_table::{
    process_create_table,
    CreateTableAndCommitmentMetadata,
    OnChainTableToTableCommitmentFn,
};

/// Create table from snapshot processing.
mod create_table_from_snapshot;
pub use create_table_from_snapshot::{
    process_create_table_from_snapshot,
    ProcessCreateTableFromSnapshotError,
};

/// Insert statement processing and commitment updates.
mod insert;
pub use insert::{
    process_insert,
    AppendOnChainTableError,
    InsertAndCommitmentMetadata,
    ProcessInsertError,
};

#[cfg(feature = "proptest")]
pub mod proptest;
