#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

/// All pallet items, built with `frame_support`.
#[allow(clippy::manual_inspect)]
#[frame_support::pallet]
pub mod pallet {
    use core::marker::PhantomData;

    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use sxt_core::system_contracts::ContractInfo;

    use super::*;

    /// The system contracts pallet.
    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// The system contracts pallet's configuration trait.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The system contracts pallet's runtime event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }

    /// Storage for basic information about the staking contract.
    #[pallet::storage]
    #[pallet::getter(fn staking_contract)]
    pub type StakingContract<T> = StorageValue<_, ContractInfo, ValueQuery>;

    /// Storage for basic information about the messaging contract.
    #[pallet::storage]
    #[pallet::getter(fn messaging_contract)]
    pub type MessagingContract<T> = StorageValue<_, ContractInfo, ValueQuery>;

    /// Genesis configuration struct for the system conracts pallet.
    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        staking_contract: ContractInfo,
        messaging_contract: ContractInfo,
        _marker: PhantomData<T>,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            // These should be explicitly defined explicitly per chain, defaults should only be
            // used for testing.
            let staking_contract = ContractInfo::default();
            let messaging_contract = ContractInfo::default();

            GenesisConfig {
                staking_contract,
                messaging_contract,
                _marker: PhantomData,
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            StakingContract::<T>::put(self.staking_contract);
            MessagingContract::<T>::put(self.messaging_contract);
        }
    }

    /// Events that the system contracts pallet can emit.
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// The staking contract information has been updated.
        StakingContractUpdated {
            /// The new contract info.
            contract_info: ContractInfo,
        },
        /// The messaging contract information has been updated.
        MessagingContractUpdated {
            /// The new contract info.
            contract_info: ContractInfo,
        },
    }

    /// The system contracts pallet's dispatchable functions ([`Call`]s).
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Sudo call for setting the stored staking contract information.
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(0, 0))]
        pub fn set_staking_contract(
            origin: OriginFor<T>,
            contract_info: ContractInfo,
        ) -> DispatchResult {
            ensure_root(origin)?;

            StakingContract::<T>::put(contract_info);

            Self::deposit_event(Event::StakingContractUpdated { contract_info });

            Ok(())
        }

        /// Sudo call for setting the stored messaging contract information.
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(0, 0))]
        pub fn set_messaging_contract(
            origin: OriginFor<T>,
            contract_info: ContractInfo,
        ) -> DispatchResult {
            ensure_root(origin)?;

            MessagingContract::<T>::put(contract_info);

            Self::deposit_event(Event::MessagingContractUpdated { contract_info });

            Ok(())
        }
    }
}
