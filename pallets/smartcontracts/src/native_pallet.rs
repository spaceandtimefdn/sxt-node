/// Wrap the pallet type to use the api object
pub type Pallet<T> = crate::pallet::Pallet<T, Api>;

/// Wrap event type to use the Api object
pub type Event<T> = crate::pallet::Event<T, Api>;

/// Wrap the error type to use the Api object
pub type Error<T> = crate::pallet::Error<T, Api>;

use native_api::Api;

/// Rexport hidden attributes
pub use crate::pallet::{
    __substrate_call_check,
    __substrate_event_check,
    tt_default_parts,
    tt_default_parts_v2,
    tt_error_token,
};
