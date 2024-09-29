//! TODO: add docs
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
    use super::*;
    use frame_support::pallet_prelude::{StorageDoubleMap, *};
    use frame_support::Blake2_128Concat;
    use frame_system::pallet_prelude::*;
    use sxt_core::permissions::*;
    use sxt_core::tables::{
        CreateStatement, IndexerMode, Source, SourceAndMode, TableIdentifier, TableName,
        TableNamespace, UpdateTableList,
    };

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_permissions::Config {
        /// TODO: add docs
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// TODO: add docs
        type WeightInfo: WeightInfo;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// The schema for a table has been updated
        SchemaUpdated(SourceAndMode, UpdateTableList),
    }

    /// A double map connecting an identifier (name, namespace) and a (source, mode) to a Schema, allowing us to interate over all tables in a namespace
    #[pallet::storage]
    #[pallet::getter(fn identifiers)]
    pub type Identifiers<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        Source,
        Blake2_128Concat,
        IndexerMode,
        TableIdentifier,
    >;

    #[pallet::storage]
    #[pallet::getter(fn schemas)]
    pub type Schemas<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        TableNamespace,
        Blake2_128Concat,
        TableName,
        CreateStatement,
    >;

    #[pallet::error]
    pub enum Error<T> {
        /// There was an error deserializing the Arrow schema
        ArrowDeserializationError,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::update_tables())]
        /// TODO: add docs
        pub fn update_tables(
            origin: OriginFor<T>,
            source_and_mode: SourceAndMode,
            tables: UpdateTableList,
        ) -> DispatchResult {
            pallet_permissions::Pallet::<T>::ensure_root_or_permissioned(
                origin,
                &PermissionLevel::TablesPallet(TablesPalletPermission::EditSchema),
            )?;

            for (identifier, statement) in tables.clone() {
                let SourceAndMode { source, mode } = source_and_mode.clone();
                Identifiers::<T>::insert(source, mode, identifier.clone());

                let TableIdentifier { name, namespace } = identifier.clone();
                Schemas::<T>::insert(namespace, name, statement.clone());
            }

            Self::deposit_event(Event::<T>::SchemaUpdated(source_and_mode, tables));

            Ok(())
        }
    }
}
