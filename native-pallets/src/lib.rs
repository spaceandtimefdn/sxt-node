//! Wrappers around pallets that enable them to call native code from inside the runtime
#![cfg_attr(not(feature = "std"), no_std)]

/// Native wrapper around the indexing pallet, this type should be used in the construct_runtime! macro rather than the basic pallet
pub mod native_pallet_indexing {
    use native_api::NativeApi;
    use sxt_core::native::{NativeError, OnChainTableBytes};

    /// What we will implement our API on
    pub struct Api;

    impl NativeApi for Api {
        fn record_batch_to_onchain(
            row_data: sxt_core::native::RowData,
        ) -> Result<OnChainTableBytes, NativeError> {
            native::interface::record_batch_to_onchain(row_data)
        }
    }

    /// Do not change below this comment.
    /// Wrap the pallet type to use the api object
    pub type Pallet<T> = pallet_indexing::Pallet<T, Api>;
    
    /// Wrap event type to use the Api object
    pub type Event<T> = pallet_indexing::Event<T, Api>;
    
    /// Wrap the error type to use the Api object
    pub type Error<T> = pallet_indexing::Error<T, Api>;

    /// Rexport hidden attributes
    pub use pallet_indexing::{
        __substrate_call_check, __substrate_event_check, tt_default_parts, tt_default_parts_v2,
        tt_error_token,
    };
}
