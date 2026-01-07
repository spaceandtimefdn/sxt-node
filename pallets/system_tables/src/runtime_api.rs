//! Runtime APIs for reading from pallet-system-tables.

use alloc::vec::Vec;

use codec::FullCodec;
use polkadot_sdk::sp_api;
use sxt_core::system_tables::ClaimedUnstake;
sp_api::decl_runtime_apis! {
    /// Runtime APIs for reading from pallet-system-tables.
    pub trait SystemTablesApi<AccountId, BlockNumber, CurrencyBalance> where AccountId: FullCodec, BlockNumber: FullCodec, CurrencyBalance: FullCodec {
        /// Returns a list of all currently-processing claimed unstakes.
        fn claimed_unstakes() -> Vec<ClaimedUnstake<AccountId, BlockNumber, CurrencyBalance>>;
    }
}
