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
    use commitment_sql::CreateTableAndCommitmentMetadata;
    use frame_support::pallet_prelude::{StorageDoubleMap, *};
    use frame_support::Blake2_128Concat;
    use frame_system::pallet_prelude::*;
    use proof_of_sql_commitment_map::generic_over_commitment::{ConcreteType, OptionType};
    use proof_of_sql_commitment_map::{
        PerCommitmentScheme,
        TableCommitmentBytes,
        TableCommitmentBytesPerCommitmentScheme,
    };
    use sp_runtime::Vec;
    use sxt_core::permissions::*;
    use sxt_core::tables::{
        create_statement_to_sqlparser,
        sqlparser_to_create_statement,
        CreateStatement,
        IndexerMode,
        SnapshotUrl,
        Source,
        SourceAndMode,
        TableIdentifier,
        TableName,
        TableNamespace,
        UpdateTableList,
    };

    use super::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config:
        frame_system::Config + pallet_permissions::Config + pallet_commitments::Config
    {
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
        TablesCreatedWithCommitments {
            /// The source and mode for the included tables (i.e. Ethereum Core)
            source_and_mode: SourceAndMode,
            /// A list of tables and their DDL Statements
            table_list: CreateTableList,
        },
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
        TableCommitmentBytesPerCommitmentScheme,
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

        /// Failed to parse Create Statement DDL
        CreateStatementParseError,
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

            let tables = tables
                .into_iter()
                .map(|(identifier, statement, commit, snapshot)| {
                    Self::insert_schema(
                        source_and_mode.clone(),
                        identifier.clone(),
                        statement.clone(),
                    );

                    let statement_with_metadata = Self::insert_initial_commitment(
                        identifier.clone(),
                        statement,
                        commit.clone(),
                        snapshot.clone(),
                    )?;

                    Ok((identifier, statement_with_metadata, commit, snapshot))
                })
                .collect::<Result<Vec<_>, DispatchError>>()?
                .try_into()
                .expect("iterator should still have < MAX_TABLES_PER_SCHEMA elements");

            Self::deposit_event(Event::<T>::TablesCreatedWithCommitments {
                source_and_mode,
                table_list: tables,
            });

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

        /// Insert the initial commit for this table using the commitments-sql pallet.
        pub fn insert_initial_commitment(
            ident: TableIdentifier,
            statement: CreateStatement,
            commit: TableCommitmentBytesPerCommitmentScheme,
            snapshot: SnapshotUrl,
        ) -> Result<CreateStatement, DispatchError> {
            let create_table = create_statement_to_sqlparser(statement)
                .map_err(|_| Error::<T>::CreateStatementParseError)?;

            let CreateTableAndCommitmentMetadata { table_with_meta_columns, .. } = pallet_commitments::Pallet::<T>::process_create_table_from_snapshot_and_initiate_commitments(
                create_table,
                commit,
            )?;

            let statement_with_metadata = sqlparser_to_create_statement(table_with_meta_columns)
                .map_err(|_| Error::<T>::CreateStatementParseError)?;

            Snapshots::<T>::insert(ident, snapshot);

            Ok(statement_with_metadata)
        }
    }

    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        tables: Vec<(
            SourceAndMode,
            TableIdentifier,
            CreateStatement,
            TableCommitmentBytesPerCommitmentScheme,
            SnapshotUrl,
        )>,
        _marker: PhantomData<T>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            for (sm, ident, stmnt, commit, snapshot) in self.tables.iter() {
                pallet::Pallet::<T>::insert_schema(sm.clone(), ident.clone(), stmnt.clone());
                pallet::Pallet::<T>::insert_initial_commitment(
                    ident.clone(),
                    stmnt.clone(),
                    commit.clone(),
                    snapshot.clone(),
                )
                .unwrap();
            }
        }
    }
}
