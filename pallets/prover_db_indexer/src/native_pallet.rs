//! Type aliases that pin our pallet's instance parameter to
//! `native_api::Api`, so `construct_runtime!` in the runtime can refer
//! to a single-type-parameter `Pallet<Runtime>` form.

use native_api::Api;

/// Wrap the pallet type to use the `Api` instance.
pub type Pallet<T> = crate::pallet::Pallet<T, Api>;

/// Wrap the event type to use the `Api` instance.
pub type Event<T> = crate::pallet::Event<T, Api>;

pub use crate::pallet::{
    __substrate_call_check,
    __substrate_event_check,
    tt_default_parts,
    tt_default_parts_v2,
    tt_error_token,
};
