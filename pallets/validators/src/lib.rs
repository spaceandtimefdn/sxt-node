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
    use sp_runtime::traits::{Convert, Zero};
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
        /// Add a new validator.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::add_validator())]
        pub fn add_validator(origin: OriginFor<T>, validator_id: T::ValidatorId) -> DispatchResult {
            ensure_root(origin)?;

            Self::do_add_validator(validator_id.clone())?;

            Ok(())
        }

        /// Remove a validator.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::remove_validator())]
        pub fn remove_validator(
            origin: OriginFor<T>,
            validator_id: T::ValidatorId,
        ) -> DispatchResult {
            ensure_root(origin)?;

            Self::do_remove_validator(validator_id.clone())?;

            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        fn do_add_validator(validator_id: T::ValidatorId) -> DispatchResult {
            ensure!(
                !<Validators<T>>::get().contains(&validator_id),
                Error::<T>::DuplicateValidatorError
            );

            let mut v = Validators::<T>::get();
            ensure!(
                v.try_push(validator_id.clone()).is_ok(),
                Error::<T>::ErrorPushingValidatorListError
            );

            Validators::<T>::put(v);

            Self::deposit_event(Event::ValidatorAdditionInitiated(validator_id.clone()));

            Ok(())
        }

        fn do_remove_validator(validator_id: T::ValidatorId) -> DispatchResult {
            let mut validators = <Validators<T>>::get();

            ensure!(
                validators.len().saturating_sub(1) as u32 >= 3,
                Error::<T>::TooLowValidatorCountError
            );

            validators.retain(|v| *v != validator_id);

            <Validators<T>>::put(validators);

            Self::deposit_event(Event::ValidatorRemovalInitiated(validator_id.clone()));

            Ok(())
        }
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        /// Validators read from the chain spec
        pub initial_validators: BoundedVec<T::ValidatorId, ConstU32<100>>,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            GenesisConfig {
                initial_validators: BoundedVec::new(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            assert!(
                <Validators<T>>::get().is_empty(),
                "Validators are already initialized!"
            );
            <Validators<T>>::put(&self.initial_validators);
        }
    }

    impl<T: Config> pallet_session::SessionManager<T::ValidatorId> for Pallet<T> {
        fn new_session(_new_index: u32) -> Option<Vec<T::ValidatorId>> {
            Some(Self::validators().into())
        }

        fn end_session(_end_index: u32) {}

        fn start_session(_start_index: u32) {}
    }

    impl<T: Config> EstimateNextSessionRotation<BlockNumberFor<T>> for Pallet<T> {
        fn average_session_length() -> BlockNumberFor<T> {
            Zero::zero()
        }

        fn estimate_current_session_progress(
            _now: BlockNumberFor<T>,
        ) -> (Option<sp_runtime::Permill>, sp_weights::Weight) {
            (None, Zero::zero())
        }

        fn estimate_next_session_rotation(
            _now: BlockNumberFor<T>,
        ) -> (Option<BlockNumberFor<T>>, sp_weights::Weight) {
            (None, Zero::zero())
        }
    }

    /// Account Identity operation
    pub struct IdentityOf<T>(sp_std::marker::PhantomData<T>);

    impl<T: Config> Convert<T::ValidatorId, Option<T::ValidatorId>> for IdentityOf<T> {
        fn convert(account: T::ValidatorId) -> Option<T::ValidatorId> {
            Some(account)
        }
    }

    impl<T: Config> ValidatorSet<T::ValidatorId> for Pallet<T> {
        type ValidatorId = T::ValidatorId;
        type ValidatorIdOf = IdentityOf<T>;

        fn session_index() -> sp_staking::SessionIndex {
            pallet_session::Pallet::<T>::current_index()
        }

        fn validators() -> Vec<T::ValidatorId> {
            pallet_session::Pallet::<T>::validators()
        }
    }

    impl<T: Config> ValidatorSetWithIdentification<T::ValidatorId> for Pallet<T> {
        type Identification = T::ValidatorId;
        type IdentificationOf = IdentityOf<T>;
    }
}
