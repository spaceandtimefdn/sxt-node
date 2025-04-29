//! Smart Contract Storage Pallet
//! This pallet allows storing and managing smart contract data using a `StorageDoubleMap`.
//! Users can add and remove smart contracts associated with a given `Source`.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::large_enum_variant)]

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
    use frame_support::pallet_prelude::{OptionQuery, StorageDoubleMap, *};
    use frame_support::Blake2_128Concat;
    use frame_system::pallet_prelude::*;
    use pallet_tables::UpdateTableList;
    use scale_info::prelude::vec::Vec;
    use sxt_core::permissions::{PermissionLevel, SmartContractsPalletPermission};
    use sxt_core::smartcontracts::{Contract, ContractAddress};
    use sxt_core::tables::{Source, TableIdentifier, TableType};

    use super::*;

    /// Pallet structure (marker type)
    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Pallet Configuration Trait
    #[pallet::config]
    pub trait Config:
        frame_system::Config + pallet_permissions::Config + pallet_tables::Config
    {
        /// The overarching runtime event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// A type representing the weights required by dispatchable functions of this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::storage]
    #[pallet::getter(fn normal_contracts)]
    pub type ContractStorage<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        Source,
        Blake2_128Concat,
        ContractAddress,
        Contract,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn contract_tables)]
    pub type ContractTables<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        Source,
        Blake2_128Concat,
        ContractAddress,
        BoundedVec<(TableIdentifier, TableType), ConstU32<1024>>, // adjust max tables as needed
        OptionQuery,
    >;

    /// Events for the Pallet
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A smart contract was added to storage.
        SmartContractAdded {
            /// owner
            owner: Option<T::AccountId>,
            /// Source chain
            source: Source,
            /// Address
            address: ContractAddress,
        },

        /// A smart contract was removed from storage.
        SmartContractRemoved {
            /// owner
            owner: Option<T::AccountId>,
            /// Source chain
            source: Source,
            /// Address
            address: ContractAddress,
        },
    }

    /// Errors for the Pallet (Not used yet but reserved for future use)
    #[pallet::error]
    pub enum Error<T> {
        /// A contract already exists for the source and address you requested
        ExistingContractError,

        /// Too many tables were attempted to be created for this smart contract
        TooManyTables,

        /// The smart contract is missing its target schema
        MissingTargetSchema,

        /// The smart contract is missing its ddl statement
        MissingDdlStatement,
    }

    /// Callable Functions (Extrinsics)
    #[pallet::call]
    impl<T: Config> Pallet<T> {
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
            address: ContractAddress,
        ) -> DispatchResult {
            // Ensure the caller is a signed user with proper permissions
            let owner = pallet_permissions::Pallet::<T>::ensure_root_or_permissioned(
                origin.clone(),
                &PermissionLevel::SmartContractsPallet(SmartContractsPalletPermission::UpdateABI),
            )?;

            // Remove the contract from storage
            ContractStorage::<T>::remove(&source, &address);

            if let Some(table_ids) = ContractTables::<T>::take(&source, &address) {
                for (ident, table_type) in table_ids {
                    pallet_tables::Pallet::<T>::drop_single_table(table_type, ident.clone())?;
                    pallet_tables::Pallet::<T>::remove_commits(ident);
                }
            }

            // Emit an event indicating the contract was removed
            Self::deposit_event(Event::SmartContractRemoved {
                owner,
                source,
                address,
            });

            Ok(())
        }

        /// Adds a new smart contract and its associated indexing tables to the chain.
        ///
        /// This function is permissioned: it can only be called by a signed account or `Root` origin
        /// with `SmartContractsPallet::UpdateABI` permission. It stores the provided smart contract,
        /// verifies it doesn’t already exist, emits a `SmartContractAdded` event, and registers any
        /// associated indexing tables via `pallet_tables::create_tables_inner`.
        ///
        /// # Parameters
        /// - `origin`: Must be either `Root` or a signed user with appropriate smart contract permissions.
        /// - `contract`: The [`Contract`] to be added. Can be a normal or proxy contract.
        /// - `tables`: The list of [`UpdateTable`] entries associated with this contract. Each entry defines
        ///    a table to be created (including schema, DDL, and type).
        ///
        /// # Emits
        /// - [`Event::SmartContractAdded`] — when the contract is successfully stored.
        /// - [`Event::SchemaUpdated`] — for each table added via `pallet_tables`.
        ///
        /// # Errors
        /// - [`Error::ExistingContractError`] — if a contract with the same `source` and `address` already exists.
        /// - Any error from:
        ///     - [`pallet_permissions::Pallet::ensure_root_or_permissioned`] if origin is unauthorized.
        ///     - [`pallet_tables::Pallet::create_tables_inner`] if any table creation fails.
        #[pallet::call_index(2)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::add_smartcontract())]
        pub fn add_smartcontract(
            origin: OriginFor<T>,
            contract: Contract,
            tables: UpdateTableList,
        ) -> DispatchResult {
            // Ensure the caller is a signed user with proper permissions
            let owner = pallet_permissions::Pallet::<T>::ensure_root_or_permissioned(
                origin.clone(),
                &PermissionLevel::SmartContractsPallet(SmartContractsPalletPermission::UpdateABI),
            )?;

            let (source, address, target_schema, ddl_statement) = match contract.clone() {
                Contract::Normal(normal_contract) => (
                    normal_contract.details.source,
                    normal_contract.details.address,
                    normal_contract.details.target_schema,
                    normal_contract.details.ddl_statement,
                ),
                Contract::Proxy(proxy_contract) => (
                    proxy_contract.details.source,
                    proxy_contract.details.address,
                    proxy_contract.details.target_schema,
                    proxy_contract.details.ddl_statement,
                ),
            };

            ensure!(
                !ContractStorage::<T>::contains_key(source.clone(), address.clone()),
                Error::<T>::ExistingContractError
            );

            ContractStorage::<T>::insert(source.clone(), address.clone(), contract);

            let table_ids: BoundedVec<_, _> = tables
                .iter()
                .map(|t| (t.ident.clone(), t.table_type.clone()))
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| Error::<T>::TooManyTables)?; // Add this error variant if needed

            ContractTables::<T>::insert(&source, &address, table_ids);

            let target_schema = target_schema.ok_or(Error::<T>::MissingTargetSchema)?;
            let ddl_statement = ddl_statement.ok_or(Error::<T>::MissingDdlStatement)?;

            pallet_tables::Pallet::<T>::create_namespace(
                origin.clone(),
                target_schema,
                0,
                ddl_statement,
                TableType::SCI,
                source.clone(),
            )?;

            Self::deposit_event(Event::SmartContractAdded {
                owner,
                source,
                address,
            });

            pallet_tables::Pallet::<T>::create_tables_inner(origin, tables)?;
            Ok(())
        }
    }
}
