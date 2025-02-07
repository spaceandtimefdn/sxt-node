//! Validators pallet
#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;
pub use weights::*;

/// Pallet implementation
#[allow(clippy::manual_inspect)]
#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_support::traits::{
        EstimateNextSessionRotation,
        ValidatorSet,
        ValidatorSetWithIdentification,
    };
    use frame_system::pallet_prelude::*;
    use pallet_session::{KeyOwner, NextKeys};
    use sp_runtime::traits::{Convert, OpaqueKeys, Zero};
    use sp_std::vec::Vec;

    use super::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_session::Config {
        /// The overarching runtime event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// A type representing the weights required by the dispatchables of this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::storage]
    pub type Something<T> = StorageValue<_, u32>;

    #[pallet::storage]
    #[pallet::getter(fn validators)]
    pub type Validators<T: Config> =
        StorageValue<_, BoundedVec<T::ValidatorId, ConstU32<100>>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn offline_validators)]
    pub type OfflineValidators<T: Config> =
        StorageValue<_, BoundedVec<T::ValidatorId, ConstU32<100>>, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// New validator addition initiated. Effective in ~2 sessions.
        ValidatorAdditionInitiated(T::ValidatorId),

        /// Validator removal initiated. Effective in ~2 sessions.
        ValidatorRemovalInitiated(T::ValidatorId),
    }

    /// Errors that can be returned by this pallet.
    #[pallet::error]
    pub enum Error<T> {
        /// The validator id is already registered
        DuplicateValidatorError,

        /// Removing this validator will drop the total validators below the acceptable threshold
        TooLowValidatorCountError,

        /// Error adding a new item to the list
        ErrorPushingValidatorListError,
    }

    /// The pallet's dispatchable functions ([`Call`]s).
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Onboard a validator while providing the session keys. Requires Sudo
        #[pallet::call_index(2)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::remove_validator())]
        pub fn onboard_validator_with_keys(
            origin: OriginFor<T>,
            validator_id: T::ValidatorId,
            session_keys: T::Keys,
        ) -> DispatchResult {
            // Sudo calls only
            ensure_root(origin)?;

            let old_keys = pallet_session::NextKeys::<T>::get(&validator_id);

            for id in <T as pallet_session::Config>::Keys::key_ids() {
                let key = session_keys.get_raw(*id);

                // ensure keys are without duplication.
                ensure!(
                    pallet_session::KeyOwner::<T>::get((id, key))
                        .map_or(true, |owner| owner == validator_id),
                    pallet_session::Error::<T>::DuplicatedKey
                );
            }

            for id in T::Keys::key_ids() {
                let key = session_keys.get_raw(*id);

                if let Some(old) = old_keys.as_ref().map(|k| k.get_raw(*id)) {
                    if key == old {
                        continue;
                    }
                    pallet_session::KeyOwner::<T>::remove((id, old));
                }

                pallet_session::KeyOwner::<T>::insert((id, key), validator_id.clone());
            }
            NextKeys::<T>::insert(validator_id.clone(), session_keys);
            Ok(())
        }
    }
}
