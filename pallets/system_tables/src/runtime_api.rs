//! Runtime APIs for reading from pallet-system-tables.

use sxt_core::system_tables::ClaimedUnstake;

sp_api::decl_runtime_apis! {
    /// Runtime APIs for reading from pallet-tables.
    pub trait SystemTablesApi<T: crate::Config> {
        /// Returns the schema for the given table identifier, in the form of a simple mapping
        /// between column name and type.
        fn claimed_unstakes() -> Vec<ClaimedUnstake<T>>;
    }
}
