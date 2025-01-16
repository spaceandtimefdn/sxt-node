//! weights generated using standard benchmark workflow


#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::Weight};
use core::marker::PhantomData;

pub trait WeightInfo {
	fn register_key() -> Weight;
	fn unregister_key() -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	/// Storage: `Keystore::Keys` (r:1 w:1)
	/// Proof: `Keystore::Keys` (`max_values`: None, `max_size`: Some(82), added: 2557, mode: `MaxEncodedLen`)
	fn register_key() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `3547`
		// Minimum execution time: 539_901_000 picoseconds.
		Weight::from_parts(555_613_000, 0)
			.saturating_add(Weight::from_parts(0, 3547))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: `Keystore::Keys` (r:1 w:1)
	/// Proof: `Keystore::Keys` (`max_values`: None, `max_size`: Some(82), added: 2557, mode: `MaxEncodedLen`)
	fn unregister_key() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `89`
		//  Estimated: `3547`
		// Minimum execution time: 7_623_000 picoseconds.
		Weight::from_parts(8_108_000, 0)
			.saturating_add(Weight::from_parts(0, 3547))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	/// register key
	fn register_key() -> Weight {
		Weight::from_parts(555_613_000, 0)
			.saturating_add(Weight::from_parts(0, 3547))
	}
	/// unregister_key
	fn unregister_key() -> Weight {
		Weight::from_parts(8_108_000, 0)
			.saturating_add(Weight::from_parts(0, 3547))
	}
}
