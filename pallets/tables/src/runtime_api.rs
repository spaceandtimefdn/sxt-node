//! Runtime APIs for reading from pallet-tables.

use sxt_core::tables::{GetTableSchemaError, TableIdentifier, TableSchema};

sp_api::decl_runtime_apis! {
    /// Runtime APIs for reading from pallet-tables.
    pub trait TablesApi {
        /// Returns the schema for the given table identifier, in the form of a simple mapping
        /// between column name and type.
        fn table_schema(table_identifier: TableIdentifier) -> Result<TableSchema, GetTableSchemaError>;
    }
}
