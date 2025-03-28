//! TODO generate real weights
#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weight functions needed for pallet_template.
pub trait WeightInfo {
	/// dummy comment will be updated when weights are calculated
	fn add_smartcontract() -> Weight;
	/// dummy comment
	fn remove_smartcontract() -> Weight;
}

/// Weights for pallet_template using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn add_smartcontract() -> Weight {
		Weight::from_parts(0, 0)
	}

	fn remove_smartcontract() -> Weight {
		Weight::from_parts(0, 0 )
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	fn add_smartcontract() -> Weight {
		Weight::from_parts(0, 0)
	}

	fn remove_smartcontract() -> Weight {
		Weight::from_parts(0, 0 )
	}
}