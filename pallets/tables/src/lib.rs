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
    use frame_support::pallet_prelude::{StorageDoubleMap, ValueQuery, *};
    use frame_support::Blake2_128Concat;
    use frame_system::pallet_prelude::*;
    use proof_of_sql_commitment_map::TableCommitmentBytesPerCommitmentScheme;
    use sp_runtime::Vec;
    use sxt_core::permissions::*;
    use sxt_core::tables::{
        create_statement_to_sqlparser,
        sqlparser_to_create_statement,
        ColumnUuidList,
        CreateStatement,
        GenesisTable,
        GenesisTableList,
        IdentifierList,
        IndexerMode,
        InsertQuorumSize,
        RawGenesisTable,
        SnapshotUrl,
        Source,
        SourceAndMode,
        TableIdentifier,
        TableName,
        TableNamespace,
        TableUuid,
        TableVersion,
        UpdateTableList,
    };
    use sxt_core::ByteString;

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

    /// A Map of Column UUIDs by Table Identifier and Version
    #[pallet::storage]
    #[pallet::getter(fn column_versions)]
    pub type ColumnVersions<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        TableIdentifier,
        Blake2_128Concat,
        TableVersion,
        ColumnUuidList,
        ValueQuery,
    >;

    /// A Map of Table UUID by Table Identifier and Version
    #[pallet::storage]
    #[pallet::getter(fn table_versions)]
    pub type TableVersions<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        TableIdentifier,
        Blake2_128Concat,
        TableVersion,
        TableUuid,
        ValueQuery,
    >;

    /// A double map connecting an identifier (name, namespace) and a (source, mode) to a Schema, allowing us to interate over all tables in a namespace
    /// ValueQuery is used so when we insert the identifiers if none have been set we get an empty bounded vec to append to
    #[pallet::storage]
    #[pallet::getter(fn identifiers)]
    pub type Identifiers<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        Source,
        Blake2_128Concat,
        IndexerMode,
        IdentifierList,
        ValueQuery,
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

    #[pallet::storage]
    pub type TableInsertQuorums<T: Config> =
        StorageMap<_, Blake2_128Concat, TableIdentifier, InsertQuorumSize, ValueQuery>;

    /// A table identifier, a sql statement for table creation, and an initial commitment
    pub type CreateTableCmd = (
        TableIdentifier,
        CreateStatement,
        InsertQuorumSize,
        TableCommitmentBytesPerCommitmentScheme,
        SnapshotUrl,
    );

    /// A struct to act as a wrapper around all the information required to create a table.
    #[derive(Debug, Clone, Eq, PartialEq, Encode, Decode, TypeInfo)]
    pub struct CreateTableRequest {
        /// The UUID for the table being created.
        pub table_uuid: TableUuid,
        /// The version for this table/UUID/Schema
        pub table_version: TableVersion,
        /// A list of UUIDs and their corresponding column names
        pub column_uuids: ColumnUuidList,
        /// The name and namespace of the table as a TableIdentifier
        pub table_name: TableIdentifier,
        /// The raw DDL Statement that should be used to create the table
        pub ddl: CreateStatement,
        /// The commitment for the historical data
        pub commitment: TableCommitmentBytesPerCommitmentScheme,
        /// The url of the historical data parquet files
        pub snapshot_url: SnapshotUrl,
        /// The quorum size to use for this table's indexing
        pub insert_quorum_size: InsertQuorumSize,
    }

    /// A bounded vec of create table commands, used to create tables from a known starting commit
    pub type CreateTableList =
        BoundedVec<CreateTableRequest, ConstU32<{ sxt_core::tables::MAX_TABLES_PER_SCHEMA }>>;

    #[pallet::error]
    pub enum Error<T> {
        /// There was an error deserializing the Arrow schema
        ArrowDeserializationError,

        /// Existing commit for this table identifier
        IdentifierAlreadyExists,

        /// Failed to parse Create Statement DDL
        CreateStatementParseError,

        /// The version submitted for this table already exists
        VersionAlreadyExists,

        /// Not all schemas were removed
        NotAllSchemasRemovedError,

        /// Not all insert quorums were removed
        NotAllInsertQuorumsRemovedError,

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

            let tables_with_meta_columns = tables.into_iter().map(|(identifier, statement, insert_quorum_size)| {
                    Self::insert_schema(source_and_mode.clone(), identifier.clone(), statement.clone(), insert_quorum_size);

                    let create_table = create_statement_to_sqlparser(statement)
                        .map_err(|_| Error::<T>::CreateStatementParseError)?;

                    let CreateTableAndCommitmentMetadata { table_with_meta_columns, .. } = pallet_commitments::Pallet::<T>::process_create_table_and_initiate_commitments(
                        create_table,
                    )?;

                    let statement_with_metadata = sqlparser_to_create_statement(table_with_meta_columns)
                        .map_err(|_| Error::<T>::CreateStatementParseError)?;
                    Ok((identifier, statement_with_metadata, insert_quorum_size))
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
                .map(|table| {
                    Self::insert_schema(
                        source_and_mode.clone(),
                        table.table_name.clone(),
                        table.ddl.clone(),
                        table.insert_quorum_size,
                    );

                    let statement_with_metadata = Self::insert_initial_commitment(
                        table.table_name.clone(),
                        table.ddl,
                        table.commitment.clone(),
                        table.snapshot_url.clone(),
                    )?;
                    let out = CreateTableRequest {
                        table_uuid: table.table_uuid,
                        table_version: table.table_version,
                        column_uuids: table.column_uuids,
                        table_name: table.table_name,
                        ddl: statement_with_metadata,
                        commitment: table.commitment,
                        snapshot_url: table.snapshot_url,
                        insert_quorum_size: table.insert_quorum_size,
                    };
                    Ok(out)
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
            let insert_quorum_size_res = TableInsertQuorums::<T>::clear(1000, None);

            // Fail if not empty
            ensure!(
                insert_quorum_size_res.maybe_cursor.is_none(),
                Error::<T>::NotAllInsertQuorumsRemovedError
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
                    .map(| GenesisTable { statement,  identifier, insert_quorum_size, .. }| {
                        Self::insert_schema(source_and_mode.clone(), identifier.clone(), statement.clone(), *insert_quorum_size);
                        let mut create_table = create_statement_to_sqlparser(statement.clone())
                            .map_err(|_| Error::<T>::CreateStatementParseError)?;

                        let index = create_table.columns.iter().position(|x| *x == commitment_sql::row_number_column_def()).expect("must have");
                        create_table.columns.remove(index);

                        let CreateTableAndCommitmentMetadata { table_with_meta_columns, .. } =
                            pallet_commitments::Pallet::<T>::process_create_table_and_initiate_commitments(create_table)?;
                        let statement_with_metadata = sqlparser_to_create_statement(table_with_meta_columns)
                            .map_err(|_| Error::<T>::CreateStatementParseError)?;
                        Ok((identifier.clone(), statement_with_metadata, *insert_quorum_size))
                    })
                    .collect::<Result<Vec<(TableIdentifier, CreateStatement, InsertQuorumSize)>, DispatchError>>()?;

                let table_list = UpdateTableList::try_from(tables_with_meta_columns).expect("this should always work");
                Self::deposit_event(Event::<T>::SchemaUpdated(source_and_mode.clone(), table_list));
                Ok::<(), DispatchError>(())
            })
            .collect::<Result<Vec<()>, DispatchError>>()?;

            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        /// Add a UUID for this table
        pub fn insert_uuid(
            ident: TableIdentifier,
            version: u16,
            uuid: TableUuid,
            column_uuids: ColumnUuidList,
        ) -> Result<(), DispatchError> {
            if TableVersions::<T>::contains_key(&ident, version) {
                // Error, this version has already been assigned a UUID
                return Err(Error::<T>::VersionAlreadyExists.into());
            }

            TableVersions::<T>::set(&ident, version, uuid);
            ColumnVersions::<T>::set(&ident, version, column_uuids);

            Ok(())
        }

        /// Uodate the schema and commitment for a table and source and mode combo
        pub fn insert_schema(
            sm: SourceAndMode,
            ident: TableIdentifier,
            stmnt: CreateStatement,
            insert_quorum_size: InsertQuorumSize,
        ) {
            let SourceAndMode { source, mode } = sm.clone();
            let mut identifiers = Identifiers::<T>::get(source.clone(), mode.clone());

            identifiers.try_push(ident.clone());
            Identifiers::<T>::insert(source, mode, identifiers);

            let TableIdentifier { name, namespace } = ident.clone();
            Schemas::<T>::insert(namespace, name, stmnt.clone());

            TableInsertQuorums::<T>::insert(ident, insert_quorum_size);
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

        /// Create a new table with an empty commitment
        pub fn insert_table_with_empty_commit(
            ident: TableIdentifier,
            statement: CreateStatement,
            snapshot: SnapshotUrl,
        ) -> Result<CreateStatement, DispatchError> {
            let create_table = create_statement_to_sqlparser(statement)
                .map_err(|_| Error::<T>::CreateStatementParseError)?;

            let CreateTableAndCommitmentMetadata {
                table_with_meta_columns,
                ..
            } = pallet_commitments::Pallet::<T>::process_create_table_and_initiate_commitments_with_dynamic_dory(
                create_table,
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
        tables: Vec<(RawGenesisTable, TableCommitmentBytesPerCommitmentScheme)>,
        tables_without_commits: Vec<RawGenesisTable>,
        _marker: PhantomData<T>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            let tables_with_meta: Vec<GenesisTable> = self
                .tables
                .iter()
                .map(|(table, commitments)| {
                    pallet::Pallet::<T>::insert_schema(
                        table.source_and_mode.clone(),
                        table.table_identifier.clone(),
                        table.create_statement.clone(),
                        table.insert_quorum_size,
                    );
                    let statement = pallet::Pallet::<T>::insert_initial_commitment(
                        table.table_identifier.clone(),
                        table.create_statement.clone(),
                        commitments.clone(),
                        table.snapshot_url.clone(),
                    )
                    .unwrap();
                    GenesisTable {
                        statement,
                        insert_quorum_size: table.insert_quorum_size,
                        url: table.snapshot_url.clone(),
                        identifier: table.table_identifier.clone(),
                        table_uuid: table.table_uuid.clone(),
                        column_uuids: table.column_uuid_list.clone(),
                        version: table.table_version,
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

            let quorum = InsertQuorumSize {
                public: Some(3),
                privileged: Some(0),
            };

            let tables_without_commits: Vec<GenesisTable> = self
                .tables_without_commits
                .iter()
                .map(|table| {
                    pallet::Pallet::<T>::insert_schema(
                        table.source_and_mode.clone(),
                        table.table_identifier.clone(),
                        table.create_statement.clone(),
                        quorum,
                    );
                    let statement = pallet::Pallet::<T>::insert_table_with_empty_commit(
                        table.table_identifier.clone(),
                        table.create_statement.clone(),
                        table.snapshot_url.clone(),
                    )
                    .unwrap();
                    GenesisTable {
                        statement: table.create_statement.clone(),
                        insert_quorum_size: table.insert_quorum_size,
                        url: table.snapshot_url.clone(),
                        identifier: table.table_identifier.clone(),
                        table_uuid: table.table_uuid.clone(),
                        column_uuids: table.column_uuid_list.clone(),
                        version: table.table_version,
                    }
                })
                .collect();

            let list_with_no_commits = GenesisTableList {
                tables: BoundedVec::try_from(tables_without_commits).unwrap(),
            };

            let contract_byte_string = ByteString::try_from(
                "0x99b712919F0c2C07ad32f4c3a3742D3C6642d0A2"
                    .as_bytes()
                    .to_vec(),
            )
            .unwrap();

            GenesisTables::<T>::insert(
                SourceAndMode {
                    source: Source::Sepolia,
                    mode: IndexerMode::SmartContract(contract_byte_string),
                },
                list_with_no_commits,
            );
        }
    }
}
