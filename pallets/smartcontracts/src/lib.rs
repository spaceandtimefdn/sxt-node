//! Smart Contract Storage Pallet
//! This pallet allows storing and managing smart contract data using a `StorageDoubleMap`.
//! Users can add and remove smart contracts associated with a given `Source`.

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod weights;
pub use weights::*;

#[allow(clippy::manual_inspect)]
#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_support::Blake2_128Concat;
    use frame_system::pallet_prelude::*;
    use sxt_core::permissions::{PermissionLevel, SmartContractsPalletPermission};
    use sxt_core::smartcontracts::{ContractABI, ContractAddress};
    use sxt_core::tables::Source;

    use super::*;

    /// Pallet structure (marker type)
    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Pallet Configuration Trait
    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_permissions::Config {
        /// The overarching runtime event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// A type representing the weights required by dispatchable functions of this pallet.
        type WeightInfo: WeightInfo;
    }

    /// Storage: Smart Contracts Mapping
    ///
    /// This `StorageDoubleMap` allows mapping:
    /// - `Source` → `ContractAddress` → `ContractABI`
    #[pallet::storage]
    #[pallet::getter(fn foo)]
    pub type Contracts<T> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, Source>,
            NMapKey<Blake2_128Concat, ContractAddress>,
            NMapKey<Twox64Concat, u64>,
        ),
        ContractABI,
        ValueQuery,
    >;

    /// Events for the Pallet
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A smart contract was added to storage.
        SmartContractSet {
            /// Source chain
            source: Source,
            /// Address
            address: ContractAddress,
        },

        /// A smart contract was removed from storage.
        SmartContractRemoved {
            /// Source chain
            source: Source,
            /// Address
            address: ContractAddress,
        },
    }

    /// Errors for the Pallet (Not used yet but reserved for future use)
    #[pallet::error]
    pub enum Error<T> {}

    /// Callable Functions (Extrinsics)
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// **Set a Smart Contract Entry**
        ///
        /// Adds or updates a smart contract entry in storage.
        ///
        /// **Parameters:**
        /// - `origin`: Must be a signed account.
        /// - `source`: The `Source` identifier for the contract.
        /// - `contract_address`: The address of the smart contract.
        /// - `contract_abi`: The ABI (interface) of the smart contract.
        ///
        /// **Emits:** `SmartContractSet`
        #[pallet::call_index(0)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::set_smartcontract())]
        pub fn set_smartcontract(
            origin: OriginFor<T>,
            source: Source,
            contract_address: ContractAddress,
            version: u64,
            contract_abi: ContractABI,
        ) -> DispatchResult {
            // Ensure the caller is a signed user with proper permissions
            pallet_permissions::Pallet::<T>::ensure_root_or_permissioned(
                origin,
                &PermissionLevel::SmartContractsPallet(SmartContractsPalletPermission::UpdateABI),
            )?;

            // Insert the contract details into storage
            Contracts::<T>::insert((&source, &contract_address, version), contract_abi);

            // Emit an event indicating the contract was set
            Self::deposit_event(Event::SmartContractSet {
                source,
                address: contract_address,
            });

            Ok(())
        }

        /// **Remove a Smart Contract Entry**
        ///
        /// Deletes a smart contract entry from storage.
        ///
        /// **Parameters:**
        /// - `origin`: Must be a signed account.
        /// - `source`: The `Source` identifier for the contract.
        /// - `contract_address`: The address of the smart contract.
        ///
        /// **Emits:** `SmartContractRemoved`
        #[pallet::call_index(1)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::remove_smartcontract())]
        pub fn remove_smartcontract(
            origin: OriginFor<T>,
            source: Source,
            contract_address: ContractAddress,
            version: u64,
        ) -> DispatchResult {
            // Ensure the caller is a signed user with proper permissions
            pallet_permissions::Pallet::<T>::ensure_root_or_permissioned(
                origin,
                &PermissionLevel::SmartContractsPallet(SmartContractsPalletPermission::UpdateABI),
            )?;

            // Remove the contract from storage
            Contracts::<T>::remove((&source, &contract_address, version));

            // Emit an event indicating the contract was removed
            Self::deposit_event(Event::SmartContractRemoved {
                source,
                address: contract_address,
            });

            Ok(())
        }
    }
}
