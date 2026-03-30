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

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

/// Native pallet functionality
pub mod native_pallet;

#[allow(clippy::manual_inspect)]
#[polkadot_sdk::frame_support::pallet]
pub mod pallet {

    use native_api::NativeApi;
    use pallet_tables::UpdateTableList;
    use polkadot_sdk::frame_support::pallet_prelude::{OptionQuery, StorageDoubleMap, *};
    use polkadot_sdk::frame_support::Blake2_128Concat;
    use polkadot_sdk::frame_system::pallet_prelude::*;
    use polkadot_sdk::sp_core;
    use scale_info::prelude::vec::Vec;
    use sxt_core::permissions::{PermissionLevel, SmartContractsPalletPermission};
    use sxt_core::smartcontracts::{Contract, ContractAddress};
    use sxt_core::tables::{Source, TableIdentifier, TableType};

    use super::*;

    /// Pallet structure (marker type)
    #[pallet::pallet]
    pub struct Pallet<T, I = ()>(_);

    /// Pallet Configuration Trait
    #[pallet::config]
    pub trait Config<I: 'static = ()>:
        polkadot_sdk::frame_system::Config
        + pallet_permissions::Config
        + pallet_tables::Config
        + pallet_indexing::Config<I>
    {
        /// The overarching runtime event type.
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as polkadot_sdk::frame_system::Config>::RuntimeEvent>;

        /// A type representing the weights required by dispatchable functions of this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::storage]
    #[pallet::getter(fn normal_contracts)]
    pub type ContractStorage<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
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
    pub type ContractTables<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
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
    pub enum Event<T: Config<I>, I: 'static = ()> {
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
    pub enum Error<T, I = ()> {
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
    impl<T: Config<I>, I: 'static> Pallet<T, I>
    where
        I: NativeApi,
        T::AccountId: sp_core::crypto::Ss58Codec,
    {
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
        #[pallet::weight(remove_smartcontract_weight::<T, I>(source, address))]
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
            ContractStorage::<T, I>::remove(&source, &address);

            if let Some(table_ids) = ContractTables::<T, I>::take(&source, &address) {
                for (ident, table_type) in table_ids {
                    pallet_tables::Pallet::<T>::drop_single_table(table_type, ident.clone())?;
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
        #[pallet::weight(add_smartcontract_weight::<T, I>(tables.len()))]
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

            let (source, address, target_schema, ddl_statement, starting_block) =
                match contract.clone() {
                    Contract::Normal(normal_contract) => (
                        normal_contract.details.source,
                        normal_contract.details.address,
                        normal_contract.details.target_schema,
                        normal_contract.details.ddl_statement,
                        normal_contract.details.starting_block,
                    ),
                    Contract::Proxy(proxy_contract) => (
                        proxy_contract.details.source,
                        proxy_contract.details.address,
                        proxy_contract.details.target_schema,
                        proxy_contract.details.ddl_statement,
                        proxy_contract.details.starting_block,
                    ),
                };

            ensure!(
                !ContractStorage::<T, I>::contains_key(source.clone(), address.clone()),
                Error::<T, I>::ExistingContractError
            );

            ContractStorage::<T, I>::insert(source.clone(), address.clone(), contract);

            let table_ids: BoundedVec<_, _> = tables
                .iter()
                .map(|t| (t.ident.clone(), t.table_type.clone()))
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| Error::<T, I>::TooManyTables)?; // Add this error variant if needed

            ContractTables::<T, I>::insert(&source, &address, &table_ids);

            // If a starting block is provided, insert it into pallet_indexing for each table
            if let Some(start_block) = starting_block {
                for (ident, _) in table_ids.iter() {
                    pallet_indexing::BlockNumbers::<T, I>::insert(ident, start_block);
                }
            }

            let target_schema = target_schema.ok_or(Error::<T, I>::MissingTargetSchema)?;
            let ddl_statement = ddl_statement.ok_or(Error::<T, I>::MissingDdlStatement)?;

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

            pallet_tables::Pallet::<T>::create_tables_inner(origin, tables, None, None)?;
            Ok(())
        }
    }

    fn add_smartcontract_weight<T, I>(num_tables: usize) -> Weight
    where
        T: Config<I>,
        I: NativeApi,
    {
        let zero_table_weight = <T as Config<I>>::WeightInfo::add_smartcontract_zero_tables();
        let one_table_weight = <T as Config<I>>::WeightInfo::add_smartcontract_one_table();
        let table_weight = one_table_weight.saturating_sub(zero_table_weight);

        zero_table_weight + (table_weight * num_tables as u64)
    }

    fn remove_smartcontract_weight<T, I>(source: &Source, address: &ContractAddress) -> Weight
    where
        T: Config<I>,
        I: NativeApi,
    {
        let num_tables = ContractTables::<T, I>::get(source, address)
            .map(|tables| tables.len())
            .unwrap_or_default();
        let zero_table_weight = <T as Config<I>>::WeightInfo::remove_smartcontract_zero_tables();
        let one_table_weight = <T as Config<I>>::WeightInfo::remove_smartcontract_one_table();
        let table_weight = one_table_weight.saturating_sub(zero_table_weight);

        zero_table_weight + (table_weight * num_tables as u64)
    }
}
