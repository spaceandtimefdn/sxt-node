#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

mod map;

mod column_options;
pub use column_options::InvalidColumnOptions;

mod column_type_conversion;
pub use column_type_conversion::{
    sqlparser_data_type_to_proof_of_sql_column_type,
    UnsupportedColumnType,
};

mod metadata_prefix;

mod row_number_column;
pub use row_number_column::row_number_column_def;

mod validated_create_table;
pub use validated_create_table::{InvalidCreateTable, ValidatedCreateTable};

mod create_table;
pub use create_table::{
    process_create_table,
    CreateTableAndCommitmentMetadata,
    OnChainTableToTableCommitmentFn,
};

mod create_table_from_snapshot;
pub use create_table_from_snapshot::{
    process_create_table_from_snapshot,
    ProcessCreateTableFromSnapshotError,
};

mod insert;
pub use insert::{
    process_insert,
    AppendOnChainTableError,
    InsertAndCommitmentMetadata,
    ProcessInsertError,
};
