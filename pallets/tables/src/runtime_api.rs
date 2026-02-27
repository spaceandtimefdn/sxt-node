//! Runtime APIs for reading from pallet-tables.
use codec::Codec;
use sxt_core::tables::{GetTableSchemaError, TableIdentifier, TableSchema};
use sxt_core::utils::account_id_from_table_id;

polkadot_sdk::sp_api::decl_runtime_apis! {
    #[api_version(2)]
    /// Runtime APIs for reading from pallet-tables.
    pub trait TablesApi<AccountId> where AccountId: Codec {
        /// Returns the schema for the given table identifier, in the form of a simple mapping
        /// between column name and type.
        fn table_schema(table_identifier: TableIdentifier) -> Result<TableSchema, GetTableSchemaError>;

        /// Returns the treasury account corresponding to a given table
        fn table_treasury(table_identifier: TableIdentifier) -> Option<AccountId>;
    }


}
