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
        TableNamespace, UpdateTableList, SnapshotUrl
    };
    use proof_of_sql_commitment_map::generic_over_commitment::{ConcreteType, OptionType};
    use proof_of_sql_commitment_map::{
       PerCommitmentScheme, TableCommitmentBytes,
    };

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_permissions::Config + pallet_commitments::Config {
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

        /// Tables have been created with known commitments
        TablesCreatedWithCommitments(SourceAndMode, CreateTableList),
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

    #[pallet::storage]
    pub type Snapshots<T: Config> = StorageMap<_, Blake2_128Concat, TableIdentifier, SnapshotUrl>;

    /// A table identifier, a sql statement for table creation, and an initial commitment
    pub type CreateTableCmd = (
        TableIdentifier,
        CreateStatement,
        PerCommitmentScheme<OptionType<ConcreteType<TableCommitmentBytes>>>,
        SnapshotUrl,
    );

    /// A bounded vec of create table commands, used to create tables from a known starting commit
    pub type CreateTableList =
        BoundedVec<CreateTableCmd, ConstU32<{ sxt_core::tables::MAX_TABLES_PER_SCHEMA }>>;


    #[pallet::error]
    pub enum Error<T> {
        /// There was an error deserializing the Arrow schema
        ArrowDeserializationError,

           /// Existing commit for this table identifier
        IdentifierAlreadyExists,
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
                Self::insert_schema(source_and_mode.clone(), identifier, statement);
            }

            Self::deposit_event(Event::<T>::SchemaUpdated(source_and_mode, tables));

            Ok(())
        }

        /// Create tables with a known commit and snapshot url from which data can be loaded
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::create_tables_with_snapshot_and_commitment())]
        pub fn create_tables_with_snapshot_and_commitment(
            origin: OriginFor<T>,
            source_and_mode: SourceAndMode,
            tables: CreateTableList,
        ) -> DispatchResult {
            pallet_permissions::Pallet::<T>::ensure_root_or_permissioned(
                origin,
                &PermissionLevel::TablesPallet(TablesPalletPermission::EditSchema),
            )?;

            for (identifier, statement, commit, snapshot) in tables.clone() {
                Self::insert_schema(source_and_mode.clone(), identifier.clone(), statement);
                Self::insert_initial_commitment(identifier, commit, snapshot)?;
            }

            Self::deposit_event(Event::<T>::TablesCreatedWithCommitments(
                source_and_mode,
                tables,
            ));

            Ok(())
        }
    }


    impl<T: Config> Pallet<T> {
        /// Uodate the schema and commitment for a table and source and mode combo
        pub fn insert_schema(sm: SourceAndMode, ident: TableIdentifier, stmnt: CreateStatement) {
            let SourceAndMode { source, mode } = sm.clone();
            Identifiers::<T>::insert(source, mode, ident.clone());

            let TableIdentifier { name, namespace } = ident.clone();
            Schemas::<T>::insert(namespace, name, stmnt.clone());
        }

        /// Insert the initial commit for this table identifier, return an error if the key already exists
        pub fn insert_initial_commitment(
            ident: TableIdentifier,
            commit: PerCommitmentScheme<OptionType<ConcreteType<TableCommitmentBytes>>>, 
            snapshot: SnapshotUrl,
        ) -> DispatchResult {
            pallet_commitments::Pallet::<T>::initiate_precomputed_commitments(
                ident.clone(),
                commit,
            )
            .map_err(|_| Error::<T>::IdentifierAlreadyExists)?;

            Snapshots::<T>::insert(ident, snapshot);

            Ok(())
        }
    }
}
