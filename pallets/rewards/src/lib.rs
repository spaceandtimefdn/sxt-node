//! # Rewards Pallet
//!
//! This pallet contains all utilities and logic related to rewards and paying them out on the
//! SXT Chain. It is designed to check at the beginning of every block for unpaid validator
//! rewards and payout one page of nominators(512) at a time for up to 3 validators per block until
//! all pages for all validators are paid.
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
extern crate core;
use alloc::vec::Vec;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

/// A Pallet that enables the automated payout of validator rewards each era.
#[allow(clippy::manual_inspect)]
#[frame_support::pallet]
pub mod pallet {
    // Import various useful types required by all FRAME pallets.
    use frame_support::pallet_prelude::*;
    use frame_support::weights::Weight;
    use frame_system::pallet_prelude::{BlockNumberFor, OriginFor};
    use pallet_staking::WeightInfo;

    use super::*;

    /// Rewards pallet, providing automated reward payouts for validator block rewards
    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Configuration trait for the rewards pallet
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Binding for the runtime event, typically provided by an implementation
        /// in runtime/lib.rs
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// How many payout calls may be made per block (prevents overweight).
        type MaxPayoutsPerBlock: Get<u32>;
    }

    /// The next era that we expect to pay out.
    #[pallet::storage]
    #[pallet::getter(fn next_paid_era)]
    pub type NextPaidEra<T: Config> = StorageValue<_, sp_staking::EraIndex, ValueQuery>;

    /// The account used to pay gas for distributing validator rewards.
    #[pallet::storage]
    #[pallet::getter(fn payer_account)]
    pub type PayerAccount<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    /// Errors that could occur while processing validator rewards for payout
    #[pallet::error]
    pub enum Error<T> {
        /// The payer has not been set
        NoPayerSet,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Set the accountID used to pay out rewards
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::zero())]
        pub fn set_payer_account(origin: OriginFor<T>, payer: T::AccountId) -> DispatchResult {
            // Check that the extrinsic was signed by root
            frame_system::ensure_root(origin)?;

            // Update storage.
            PayerAccount::<T>::put(payer.clone());

            // Emit an event.
            Self::deposit_event(Event::PayerUpdated { payer });

            // Return a successful `DispatchResult`
            Ok(())
        }
    }

    /// Events that can be emitted by this pallet
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// An era has been fully paid
        EraPaid {
            /// The index of the era that was paid out
            index: sp_staking::EraIndex,
        },
        /// A payout of accrued rewards was made automatically by the system
        Payout {
            /// The validator who had a page of nominators paid out
            validator: T::AccountId,
        },
        /// Events regarding status of payouts
        Status {
            /// The current era
            current_era: sp_staking::EraIndex,
            /// The era we're trying to pay out
            paying_era: sp_staking::EraIndex,
        },
        /// An error occurred paying out a particular validator
        PayoutError {
            /// The validator we were trying to pay out when the error occurred
            validator: T::AccountId,
            /// The error received
            error: DispatchError,
        },
        /// An error occurred running the payouts
        SetupError {
            /// The error received
            error: DispatchError,
        },
        /// The account used to payout rewards has been updated
        PayerUpdated {
            /// The account ID of the new payer
            payer: T::AccountId,
        },
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T>
    where
        T: pallet_staking::Config,
    {
        /// This hook is called on every block. It will check for any unpaid eras, and perform the
        /// payouts across a number of blocks until all payouts for the previous eras have been
        /// paid.
        fn on_initialize(_: BlockNumberFor<T>) -> Weight {
            let mut total_weight: Weight = Weight::zero();

            // Start by getting the last era we've paid out
            let active_era = NextPaidEra::<T>::get();

            // And the current era
            let current_era = pallet_staking::Pallet::<T>::active_era()
                .map(|i| i.index)
                .unwrap_or_default();

            total_weight.saturating_add(T::DbWeight::get().reads(2));

            if active_era >= current_era {
                // We don't need to pay anything out because we are caught up.
                return total_weight;
            }

            Self::deposit_event(Event::Status {
                current_era,
                paying_era: active_era,
            });

            let max_page_size = T::MaxExposurePageSize::get();
            total_weight.saturating_add(T::DbWeight::get().reads(1));

            let single_call_weight: Weight =
                pallet_staking::weights::SubstrateWeight::<T>::payout_stakers_alive_staked(
                    max_page_size,
                );

            // Only payout this many validators per block
            let max_payouts = T::MaxPayoutsPerBlock::get() as usize;
            total_weight.saturating_add(T::DbWeight::get().reads(1));

            // We can get all the validators that need to be rewarded by querying the list of points
            // recipients for the era
            let rewards_for_era = pallet_staking::Pallet::<T>::eras_reward_points(active_era);
            total_weight.saturating_add(T::DbWeight::get().reads(1));

            let payer = PayerAccount::<T>::get();
            total_weight.saturating_add(T::DbWeight::get().reads(1));

            let Some(payer) = payer else {
                Self::deposit_event(Event::SetupError {
                    error: Error::<T>::NoPayerSet.into(),
                });
                return total_weight;
            };

            // Here we start by filtering any validators from the list that have no pending rewards
            // for the era. After that payout a single page of Nominators for each staker up to the
            // declared MaxPayoutsPerBlock. Each page contains up to `MaxExposurePageSize` nominators
            // (which is currently 512). A safe limit seems to be 3 pages per block which translates
            // to 3 validators per block in this implementation
            let reward_weight = rewards_for_era
                .individual
                .into_iter()
                .filter(|(validator, points)| {
                    pallet_staking::EraInfo::<T>::pending_rewards(active_era, validator)
                })
                .take(max_payouts)
                .map(|(validator, points)| {
                    let origin = frame_system::RawOrigin::Signed(payer.clone()).into();
                    match pallet_staking::Pallet::<T>::payout_stakers(
                        origin,
                        validator.clone(),
                        active_era,
                    ) {
                        Ok(_) => {
                            Self::deposit_event(Event::Payout { validator });
                        }
                        Err(err) => {
                            Self::deposit_event(Event::PayoutError {
                                validator,
                                error: err.error,
                            });
                        }
                    }

                    single_call_weight
                })
                .reduce(|w1, w2| w1.saturating_add(w2))
                .unwrap_or(Weight::zero());

            if reward_weight == Weight::zero() {
                // This indicates there were no remaining payouts for this era, so we record it
                Self::deposit_event(Event::EraPaid { index: active_era });
                NextPaidEra::<T>::set(active_era + 1);
            }

            total_weight.saturating_add(reward_weight);
            total_weight
        }
    }
}
