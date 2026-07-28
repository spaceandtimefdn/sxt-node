/// Wrap the pallet type to use the api object
pub type Pallet<T> = crate::pallet::Pallet<T, Api>;

use native_api::Api;

/// Rexport hidden attributes
pub use crate::pallet::{tt_default_parts, tt_default_parts_v2, tt_error_token};
