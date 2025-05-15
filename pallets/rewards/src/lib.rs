//! # Rewards Pallet
//!
//! This pallet contains all utilities and logic related to rewards and paying them out on the
//! SXT Chain
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
extern crate core;
use alloc::string::String;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

// FRAME pallets require their own "mock runtimes" to be able to run unit tests.
#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

/// Weights for the rewards pallet
pub mod weights;
pub use weights::*;

#[allow(clippy::manual_inspect)]
#[frame_support::pallet]
pub mod pallet {
    // Import various useful types required by all FRAME pallets.
    use frame_support::pallet_prelude::*;

    use super::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config
            + pallet_session::Config
            + pallet_staking::Config<CurrencyBalance = u128>
            + pallet_balances::Config {
        /// Binding for the runtime event, typically provided by an implementation
        /// in runtime/lib.rs
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// The weight info to be used with the extrinsics provided by the pallet
        type WeightInfo: WeightInfo;
        /// How many payout calls may be made per block (prevents overweight).
        #[pallet::constant]
        type MaxPayoutsPerBlock: Get<u32>;
        /// Origin we will use when dispatching `staking::payout_stakers`.
        type PayoutOrigin: EnsureOrigin<Self::RuntimeOrigin>;
    }

    /// The last era that was being paid.
    /// (era_being_processed, next_validator_index)
    #[pallet::storage]
    pub type PayCursor<T: Config> = StorageValue<_, (sp_staking::EraIndex, u32), ValueQuery>;

    /// The last era that has been fully paid out.
    #[pallet::storage]
    #[pallet::getter(fn last_paid_era)]
    pub type LastPaidEra<T: Config> = StorageValue<_, sp_staking::EraIndex, ValueQuery>;

    /// Events that can be emitted by this pallet
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A user has successfully set a new value.
        EraPaid {
            // The index of the era that was paid out
            index: sp_staking::EraIndex,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {}

    #[pallet::hooks]
    impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> where T: pallet_staking::Config {
        /// This hook is called on every block. It will check for any unpaid eras, and perform the
        /// payouts across a number of blocks until all payouts for the previous eras have been
        /// paid.
        fn on_initialize(_: T::BlockNumber) -> Weight {

            let mut total_weight: Weight = Weight::zero();

            // Start by getting the last era we've paid out
            let last_paid_era = LastPaidEra::<T>::get();
            
            // And the current era
            let current_era = pallet_staking::Pallet::<T>::active_era()
                .map(|i| i.index)
                .unwrap_or_default();

            total_weight.saturating_add(T::DbWeight::get().reads(2));
            
            if last_paid_era >= current_era - 1 {
                // We don't need to pay anything out because we are caught up.
                return total_weight;
            }

            pallet_staking::<T>::is_rewards_claimed_with_legacy_fallback();

            let max_payouts = T::MaxPayoutsPerBlock::get();
            
            /// Get the stakers from the last unpaid era
            let stakers = pallet_staking::Pallet::<T>::get(last_paid_era);
            // let stakers = staking::ErasStakers::<T>::i(last_paid_era).take_while(|_| left_this_block > 0);
            // 
            // let call = staking::Call::<T>::payout_stakers {
            //     era: last_paid_era,
            //     validator_stash: stash.clone(),
            // };
            // 
            // // Dispatch as the configured origin (Root or a governance pallet).
            // if T::PayoutOrigin::ensure_origin(frame_system::RawOrigin::Root.into()).is_ok() {
            //     let _ = call.dispatch_bypass_filter(frame_system::RawOrigin::Root.into());
            // }
            // 
            // LastPaidEra::<T>::put(last_paid_era);

            // // Nothing to do the very first era.
            // if last_paid_era == 0 && current_era > 0 {
            //     last_paid_era = current_era - 1;
            // }
            //
            // Work through unpaid eras (normally it’s just `current_era - 1`)
            // but in case of missed blocks we continue where we left off.

            // while last_paid_era < current_era {
                // let mut left_this_block = T::MaxPayoutsPerBlock::get();
                // Walk all validator stashes for `last_era`.
                // for (stash, _) in
                //     staking::ErasStakers::<T>::drain_prefix(last_paid_era).take_while(|_| left_this_block > 0)
                // {
                //     let call = staking::Call::<T>::payout_stakers {
                //         era: last_paid_era,
                //         validator_stash: stash.clone(),
                //     };
                //     // Dispatch as the configured origin (Root or a governance pallet).
                //     if T::PayoutOrigin::ensure_origin(frame_system::RawOrigin::Root.into()).is_ok() {
                //         let _ = call.dispatch_bypass_filter(frame_system::RawOrigin::Root.into());
                //     }
                //     left_this_block -= 1;
                //     total_weight += T::DbWeight::get().writes(1);
                // }
                // if left_this_block != 0 {
                //     // All validators for this era paid, move on.
                //     last_paid_era += 1;
                //     LastPaidEra::<T>::put(last_paid_era);
                // } else {
                //     // Hit the per‑block limit, continue next block.
                //     break;
                // }
            // }
            total_weight
        }
    }
}
