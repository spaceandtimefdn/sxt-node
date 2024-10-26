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
    use frame_support::dispatch::DispatchResult;
    use frame_support::pallet_prelude::{StorageDoubleMap, *};
    use frame_support::Blake2_128Concat;
    use frame_system::pallet_prelude::*;
    use proof_of_sql_commitment_map::TableCommitmentBytesPerCommitmentScheme;
    use sp_runtime::Vec;
    use sxt_core::permissions::*;
    use sxt_core::tables::{
        create_statement_to_sqlparser,
        sqlparser_to_create_statement,
        CreateStatement,
        GenesisTable,
        GenesisTableList,
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
    #[pallet::getter(fn genesis_tables)]
    pub type GenesisTables<T: Config> =
        StorageMap<_, Blake2_128Concat, SourceAndMode, GenesisTableList>;

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

        /// Not all schemas were removed
        NotAllSchemasRemovedError,

        /// Not all commitments were removed
        NotAllCommitmentsRemovedError,
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

            let tables_with_meta_columns = tables.into_iter().map(|(identifier, statement)| {
                    Self::insert_schema(source_and_mode.clone(), identifier.clone(), statement.clone());

                    let create_table = create_statement_to_sqlparser(statement)
                        .map_err(|_| Error::<T>::CreateStatementParseError)?;

                    let CreateTableAndCommitmentMetadata { table_with_meta_columns, .. } = pallet_commitments::Pallet::<T>::process_create_table_and_initiate_commitments(
                        create_table,
                    )?;

                    let statement_with_metadata = sqlparser_to_create_statement(table_with_meta_columns)
                        .map_err(|_| Error::<T>::CreateStatementParseError)?;
                    Ok((identifier, statement_with_metadata))
                })
                .collect::<Result<Vec<_>, DispatchError>>()?
                .try_into()
                .expect("iterator should still have < MAX_TABLES_PER_SCHEMA elements");

            Self::deposit_event(Event::<T>::SchemaUpdated(
                source_and_mode,
                tables_with_meta_columns,
            ));

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

        /// Clear schemas and tables from chain state for all namespaces and identifiers
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::clear_tables())]
        pub fn clear_tables(origin: OriginFor<T>) -> DispatchResult {
            // Only sudo can call this
            ensure_root(origin)?;

            // Clear up to 1000 schemas
            let schema_res = Schemas::<T>::clear(1000, None);

            // Ensure it's been cleared, if this fails we can call it again and do the next 1000
            ensure!(
                schema_res.maybe_cursor.is_none(),
                Error::<T>::NotAllSchemasRemovedError
            );

            // Clear 1000
            let commit_res = pallet_commitments::CommitmentStorageMap::<T>::clear(1000, None);

            // Fail if not empty
            ensure!(
                commit_res.maybe_cursor.is_none(),
                Error::<T>::NotAllCommitmentsRemovedError
            );

            Ok(())
        }

        /// Attempts to recreate all tables stored in the genesis, but does not start loading from
        /// snapshot
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::create_empty_genesis_tables())]
        pub fn create_empty_genesis_tables(origin: OriginFor<T>) -> DispatchResult {
            ensure_root(origin)?;

            GenesisTables::<T>::iter()
            .map(|(source_and_mode, genesis_list)| {
                // Process the genesis list and map over each table
                let tables_with_meta_columns = genesis_list
                    .tables
                    .iter()
                    .map(|GenesisTable { statement, url, identifier }| {
                        Self::insert_schema(source_and_mode.clone(), identifier.clone(), statement.clone());
                        let mut create_table = create_statement_to_sqlparser(statement.clone())
                            .map_err(|_| Error::<T>::CreateStatementParseError)?;

                        let index = create_table.columns.iter().position(|x| *x == commitment_sql::row_number_column_def()).expect("must have");
                        create_table.columns.remove(index);

                        let CreateTableAndCommitmentMetadata { table_with_meta_columns, .. } =
                            pallet_commitments::Pallet::<T>::process_create_table_and_initiate_commitments(create_table)?;
                        let statement_with_metadata = sqlparser_to_create_statement(table_with_meta_columns)
                            .map_err(|_| Error::<T>::CreateStatementParseError)?;
                        Ok((identifier.clone(), statement_with_metadata))
                    })
                    .collect::<Result<Vec<(TableIdentifier, CreateStatement)>, DispatchError>>()?;

                let table_list = UpdateTableList::try_from(tables_with_meta_columns).expect("this should always work");
                Self::deposit_event(Event::<T>::SchemaUpdated(source_and_mode.clone(), table_list));
                Ok::<(), DispatchError>(())
            })
            .collect::<Result<Vec<()>, DispatchError>>()?;

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
            let tables_with_meta: Vec<GenesisTable> = self
                .tables
                .iter()
                .map(|(sm, ident, stmnt, commit, snapshot)| {
                    pallet::Pallet::<T>::insert_schema(sm.clone(), ident.clone(), stmnt.clone());
                    let statement = pallet::Pallet::<T>::insert_initial_commitment(
                        ident.clone(),
                        stmnt.clone(),
                        commit.clone(),
                        snapshot.clone(),
                    )
                    .unwrap();
                    GenesisTable {
                        statement,
                        url: snapshot.clone(),
                        identifier: ident.clone(),
                    }
                })
                .collect();

            let list = GenesisTableList {
                tables: BoundedVec::try_from(tables_with_meta).unwrap(),
            };

            GenesisTables::<T>::insert(
                SourceAndMode {
                    source: Source::Ethereum,
                    mode: IndexerMode::Core,
                },
                list,
            );
        }
    }
}
