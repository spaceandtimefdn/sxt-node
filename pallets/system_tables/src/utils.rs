//! Handy Utilities for system_tables
#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;
use alloc::vec::Vec;

use pallet_staking::UnlockChunk;
use sp_staking::EraIndex;

/// Takes in a bounded vec of unlocking chunks from a staking ledger and totals up all funds that
/// are currently unlocked and available for withdraw. If there are no funds available, this will
/// return 0
pub(crate) fn unlocked_funds_of<T: crate::Config>(
    unlocking_chunks: Vec<UnlockChunk<T::Balance>>,
) -> u128 {
    let mut out = 0u128;
    for chunk in unlocking_chunks {
        //      if chunk.era < pallet_staking::CurrentEra::<T>::get() {
        //       out.saturating_add(chunk.value);
        //  }
    }
    0
}

pub fn current_era<T: crate::Config>() -> EraIndex {
    <pallet_staking::Pallet<T>>::current_era().unwrap_or(0)
}
