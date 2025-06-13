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
    use core::str::{from_utf8, Utf8Error};

    use codec::alloc::borrow::ToOwned;
    use commitment_sql::CreateTableAndCommitmentMetadata;
    use frame_support::dispatch::DispatchResult;
    use frame_support::pallet_prelude::{StorageDoubleMap, ValueQuery, *};
    use frame_support::Blake2_128Concat;
    use frame_system::pallet_prelude::*;
    use proof_of_sql_commitment_map::{
        CommitmentSchemeFlags,
        TableCommitmentBytes,
        TableCommitmentBytesPerCommitmentScheme,
    };
    use scale_info::prelude::vec;
    use sp_runtime::Vec;
    use sqlparser::ast::helpers::stmt_create_table::CreateTableBuilder;
    use sqlparser::ast::{ColumnDef, ColumnOption, ColumnOptionDef, DataType, Ident};
    use sxt_core::permissions::*;
    use sxt_core::tables::{
        convert_sql_to_ignite_create_statement,
        create_statement_to_sqlparser,
        create_statement_to_sqlparser_remove_with,
        extract_schema_uuid,
        generate_column_uuid_list,
        generate_column_uuid_list2,
        generate_namespace_uuid,
        generate_table_uuid,
        sqlparser_to_create_statement,
        uuids_from_create_statement,
        uuids_from_sqlparser,
        ColumnUuidList,
        CommitmentBytes,
        CommitmentScheme,
        CreateStatement,
        IdentifierList,
        IndexerMode,
        InsertQuorumSize,
        SnapshotUrl,
        Source,
        SourceAndMode,
        TableIdentifier,
        TableName,
        TableNamespace,
        TableType,
        TableUuid,
        TableVersion,
    };
    use sxt_core::ByteString;

    use super::*;

    /// TODO: add docs
    pub type UpdateTableCmd = (
        TableIdentifier,
        CreateStatement,
        TableType,
        Option<CommitmentBytes>,
        Option<SnapshotUrl>,
        Option<CommitmentScheme>,
    );

    /// The individual information needed to create (update) a table
    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub struct UpdateTable {
        /// Table identifier (name, namespace)
        pub ident: TableIdentifier,
        /// DDL statement
        pub create_statement: CreateStatement,
        /// Table type
        pub table_type: TableType,
        /// Commitment related data
        pub commitment: CommitmentCreationCmd,
        /// Source chain
        pub source: Source,
    }

    /// A list of tables that we want to create or update
    pub type UpdateTableList = BoundedVec<UpdateTable, ConstU32<1024>>;

    /// What type of commitment to create
    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub enum CommitmentCreationCmd {
        /// From a preexisting commitment
        FromSnapshot(SnapshotUrl, TableCommitmentBytesPerCommitmentScheme),
        /// An empty commitment
        Empty(CommitmentSchemeFlags),
    }

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
        /// The namespace for a schema has been created
        NamespaceCreated {
            /// The create statement for the namespace/schema
            create_schema: CreateStatement,
            /// The version of the namespace
            version: TableVersion,
            /// The uuid of the namespace
            namespace_uuid: TableUuid,
            /// Table type
            table_type: TableType,
            /// Source
            source: Source,
        },

        /// The schema for a table has been updated
        SchemaUpdated(Option<T::AccountId>, UpdateTableList),

        /// Tables have been created with known commitments
        TablesCreatedWithCommitments {
            /// The source and mode for the included tables (i.e. Ethereum Core)
            source_and_mode: SourceAndMode,
            /// A list of tables and their DDL Statements
            table_list: CreateTableList,
        },

        /// A table has been successfully dropped
        TableDropped(Option<T::AccountId>, TableType, TableIdentifier, Source),
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

    /// A Map of Namespace/Schema UUID by Namespace and Version
    #[pallet::storage]
    #[pallet::getter(fn namespace_versions)]
    pub type NamespaceVersions<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        TableNamespace,
        Blake2_128Concat,
        TableVersion,
        TableUuid,
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

    /// Map of TableTypes to Identifiers
    #[pallet::storage]
    #[pallet::getter(fn identifiers)]
    pub type Identifiers<T: Config> =
        StorageMap<_, Blake2_128Concat, TableType, IdentifierList, ValueQuery>;

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

    #[pallet::storage]
    pub type TableSources<T: Config> =
        StorageMap<_, Blake2_128Concat, TableIdentifier, Source, ValueQuery>;

    /// Maps a table identifier to the account that created it.
    /// Only used for public/permissionless tables.
    #[pallet::storage]
    #[pallet::getter(fn table_owners)]
    pub type TableOwners<T: Config> =
        StorageMap<_, Blake2_128Concat, TableIdentifier, T::AccountId>;

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
        /// Table Type
        pub table_type: TableType,
    }

    /// A bounded vec of create table commands, used to create tables from a known starting commit
    pub type CreateTableList =
        BoundedVec<CreateTableRequest, ConstU32<{ sxt_core::tables::MAX_TABLES_PER_SCHEMA }>>;

    #[pallet::error]
    pub enum Error<T> {
        /// There was an error deserializing the Arrow schema
        ArrowDeserializationError,

        /// The provided Table Identifier was unable to be parsed
        TableIdentifierParsingError,

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

        /// The desired table could not be located
        TableNotFound,

        /// missing commitment scheme
        MissingCommitmentScheme,

        /// Error constructing a bounded vector for the given data
        BoundedVecError,

        /// Missing snapshot
        MissingSnapshot,

        /// Error parsing the schema name as utf8
        SchemaNameParseError,

        /// Error with a generated uuid
        GeneratedUuidError,

        /// Table uuid error
        TableUUIDError,

        /// Error parsing a DDL statement into Utf8
        UtfConversionError,

        /// There was an error generating a uuid
        UUIDGenerationError,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::update_tables())]
        /// TODO: add docs
        pub fn create_tables(origin: OriginFor<T>, tables: UpdateTableList) -> DispatchResult {
            Self::create_tables_inner(origin, tables)
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
                        table.table_name.clone(),
                        table.ddl.clone(),
                        table.table_type.clone(),
                        source_and_mode.source.clone(),
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
                        table_type: table.table_type,
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

        /// Used to create a new namespace/schema on chain. Stores the associated UUID and emits
        /// an event containing the CREATE statement
        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::create_namespace())]
        pub fn create_namespace(
            origin: OriginFor<T>,
            schema_name: ByteString,
            version: TableVersion,
            create_statement: CreateStatement,
            table_type: TableType,
            source: Source,
        ) -> DispatchResult {
            pallet_permissions::Pallet::<T>::ensure_root_or_permissioned(
                origin,
                &PermissionLevel::TablesPallet(TablesPalletPermission::EditSchema),
            )?;
            let raw_sql =
                from_utf8(&create_statement).map_err(|_| Error::<T>::CreateStatementParseError)?;

            let block_number = <frame_system::Pallet<T>>::block_number();

            let schema_name_s =
                from_utf8(&schema_name).map_err(|_| Error::<T>::SchemaNameParseError)?;

            let namespace_uuid = match extract_schema_uuid(raw_sql) {
                Some(uuid) => TableUuid::try_from(uuid.as_bytes().to_vec())
                    .map_err(|_| Error::<T>::TableUUIDError)?,
                None => generate_namespace_uuid(block_number.into(), schema_name_s)?,
            };

            Self::insert_namespace_uuid(schema_name, version, namespace_uuid.clone())?;

            Self::deposit_event(Event::<T>::NamespaceCreated {
                create_schema: create_statement,
                version,
                namespace_uuid,
                table_type,
                source,
            });
            Ok(())
        }

        /// Drop a single table
        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::drop_table())]
        pub fn drop_table(
            origin: OriginFor<T>,
            table_type: TableType,
            ident: TableIdentifier,
            source: Source,
        ) -> DispatchResult {
            let owner = pallet_permissions::Pallet::<T>::ensure_root_or_permissioned(
                origin.clone(),
                &PermissionLevel::TablesPallet(TablesPalletPermission::EditSchema),
            )?;

            Self::drop_single_table(table_type.clone(), ident.clone())?;
            Self::remove_commits(ident.clone());
            Self::deposit_event(Event::<T>::TableDropped(owner, table_type, ident, source));

            Ok(())
        }

        /// TODO remove this function
        #[pallet::call_index(6)]
        #[pallet::weight(<T as Config>::WeightInfo::drop_table())]
        pub fn drop_invalid_commits(
            origin: OriginFor<T>,
            ident: TableIdentifier,
        ) -> DispatchResult {
            ensure_root(origin)?;

            Self::remove_commits(ident);

            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        /// Remove commits based on identifier
        pub fn remove_commits(ident: TableIdentifier) {
            for (k1, k2, _) in pallet_commitments::CommitmentStorageMap::<T>::iter() {
                if k1 == ident {
                    pallet_commitments::CommitmentStorageMap::<T>::remove(&ident, k2);
                }
            }
        }

        /// Insert a given Namespace's UUID along with the corresponding version
        pub fn insert_namespace_uuid(
            namespace_name: TableNamespace,
            version: u16,
            namespace_uuid: TableUuid,
        ) -> Result<(), DispatchError> {
            if NamespaceVersions::<T>::contains_key(&namespace_name, version) {
                // Error, this version has already been assigned a UUID
                return Err(Error::<T>::VersionAlreadyExists.into());
            }

            NamespaceVersions::<T>::set(&namespace_name, version, namespace_uuid);

            Ok(())
        }

        /// Add a UUID for this table
        pub fn insert_table_uuid(
            ident: TableIdentifier,
            uuid: TableUuid,
            column_uuids: ColumnUuidList,
        ) -> Result<TableVersion, DispatchError> {
            let next_version = TableVersions::<T>::iter_prefix(&ident)
                .map(|(v, _)| v)
                .max()
                .map(|v| v + 1)
                .unwrap_or(0);

            // Insert table and column UUIDs at the computed version
            TableVersions::<T>::set(&ident, next_version, uuid);
            ColumnVersions::<T>::set(&ident, next_version, column_uuids);

            Ok(next_version)
        }

        /// Uodate the schema and commitment for a table and source and mode combo
        pub fn insert_schema(
            ident: TableIdentifier,
            stmnt: CreateStatement,
            table_type: TableType,
            source: Source,
        ) {
            let mut identifiers = Identifiers::<T>::get(&table_type);

            identifiers.try_push(ident.clone());
            Identifiers::<T>::insert(&table_type, identifiers);

            let TableIdentifier { name, namespace } = ident.clone();
            Schemas::<T>::insert(namespace, name, stmnt.clone());
            let quorum: InsertQuorumSize = table_type.into();

            TableInsertQuorums::<T>::insert(&ident, quorum);
            TableSources::<T>::insert(&ident, source);
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

        /// Attempts to extract UUIDs for the provided table, generating new ones if there are none
        /// present in the DDL
        pub fn get_or_generate_uuids_for_table(
            statement: CreateStatement,
            identifier: TableIdentifier,
        ) -> (TableUuid, ColumnUuidList) {
            // Check if this table statement has UUIDs embedded in it
            let Some((table_uuid, column_uuids)) = uuids_from_create_statement(statement.clone())
            else {
                // If not we generate them for the table
                let block_number = <frame_system::Pallet<T>>::block_number();
                let namespace = from_utf8(&identifier.namespace).unwrap();
                let name = from_utf8(&identifier.name).unwrap();
                return (
                    generate_table_uuid(block_number.into(), namespace, name).unwrap(),
                    generate_column_uuid_list(statement),
                );
            };

            (table_uuid, column_uuids)
        }

        /// v2
        pub fn get_or_generate_uuids_for_table2(
            raw: CreateStatement,
            identifier: TableIdentifier,
        ) -> Result<(TableUuid, ColumnUuidList), DispatchError> {
            // Try parsing the statement
            if let Ok(parsed) = create_statement_to_sqlparser(raw.clone()) {
                let (table_uuid, column_uuids) = uuids_from_sqlparser(parsed);
                let has_any = table_uuid != TableUuid::default() || !column_uuids.is_empty();
                if has_any {
                    return Ok((table_uuid, column_uuids));
                }
            }

            // Fallback: generate new UUIDs
            let block_number = <frame_system::Pallet<T>>::block_number();

            let namespace = from_utf8(&identifier.namespace)
                .map_err(|_| DispatchError::Other("Invalid UTF-8 namespace"))?;

            let name = from_utf8(&identifier.name)
                .map_err(|_| DispatchError::Other("Invalid UTF-8 name"))?;

            let table_uuid = generate_table_uuid(block_number.into(), namespace, name)?;
            let column_uuids = generate_column_uuid_list2(raw)?;

            Ok((table_uuid, column_uuids))
        }

        /// Drop a single table
        pub fn drop_single_table(table_type: TableType, ident: TableIdentifier) -> DispatchResult {
            // Retrieve the current list of table identifiers for this source and mode.
            let mut identifiers = Identifiers::<T>::get(&table_type);

            // Retain all identifiers that are not equal to `ident`
            identifiers.retain(|id| id != &ident);

            if identifiers.len() < Identifiers::<T>::get(&table_type).len() {
                Identifiers::<T>::insert(&table_type, identifiers);
            } else {
                return Err(Error::<T>::TableNotFound.into());
            }

            // Remove the schema definition.
            let TableIdentifier { name, namespace } = ident.clone();
            if Schemas::<T>::contains_key(&namespace, &name) {
                Schemas::<T>::remove(&namespace, &name);
            } else {
                return Err(Error::<T>::TableNotFound.into());
            }

            // Remove the insert quorum size entry.
            if TableInsertQuorums::<T>::contains_key(&ident) {
                TableInsertQuorums::<T>::remove(&ident);
            }

            Ok(())
        }

        /// Create a table. Exactly the same as the extrinsic but available to other pallets
        pub fn create_tables_inner(
            origin: OriginFor<T>,
            tables: UpdateTableList,
        ) -> DispatchResult {
            let owner = pallet_permissions::Pallet::<T>::ensure_root_or_permissioned(
                origin.clone(),
                &PermissionLevel::TablesPallet(TablesPalletPermission::EditSchema),
            )?;

            let tables_with_meta_columns = tables
        .into_iter()
        .map(|mut table| {
            let is_public = matches!(table.table_type, TableType::PublicPermissionless);

            // Generate or extract UUIDs
            let (table_uuid, column_uuids) = pallet::Pallet::<T>::get_or_generate_uuids_for_table2(
                table.create_statement.clone(),
                table.ident.clone(),
            )
            .map_err(|_| Error::<T>::UUIDGenerationError)?;

            Self::insert_table_uuid(table.ident.clone(), table_uuid, column_uuids)?;
            Self::insert_schema(
                table.ident.clone(),
                table.create_statement.clone(),
                table.table_type.clone(),
                table.source.clone(),
            );

            // Parse and remove WITH clause
            let (mut create_table, with_options) = create_statement_to_sqlparser_remove_with(
                table.create_statement.clone(),
            )
            .map_err(|_| Error::<T>::CreateStatementParseError)?;

            // Inject submitter column if this is a permissionless table
            if is_public {
                create_table = inject_submitter_column(create_table);
            }

            // Generate metadata
            let CreateTableAndCommitmentMetadata {
                table_with_meta_columns,
                ..
            } = match table.commitment {
                CommitmentCreationCmd::Empty(scheme) => {
                    pallet_commitments::Pallet::<T>::process_create_table_and_initiate_commitments_with_scheme(
                        create_table,
                        scheme,
                    )?
                }
                CommitmentCreationCmd::FromSnapshot(ref snapshot_url, ref per_commitment_scheme) => {
                    Snapshots::<T>::insert(table.ident.clone(), snapshot_url.clone());
                    pallet_commitments::Pallet::<T>::process_create_table_from_snapshot_and_initiate_commitments(
                        create_table,
                        per_commitment_scheme.clone(),
                    )?
                }
            };

            // Reconstruct final DDL statement
            let statement_with_metadata = sqlparser_to_create_statement(table_with_meta_columns)
                .map_err(|_| Error::<T>::CreateStatementParseError)?;

            let statement_with_metadata = from_utf8(&statement_with_metadata)
                .map_err(|_| Error::<T>::UtfConversionError)?;

            let reconstructed = match with_options {
                Some(opts) => {
                    let mut base = statement_with_metadata.trim_end_matches(';').to_owned();
                    base.push(' ');
                    base.push_str(from_utf8(&opts).map_err(|_| Error::<T>::UtfConversionError)?);
                    base.push(';');
                    base
                }
                None => {
                    let mut base = statement_with_metadata.trim_end_matches(';').to_owned();
                    base.push(';');
                    base
                }
            };

            table.create_statement = CreateStatement::try_from(reconstructed.as_bytes().to_vec())
                .map_err(|_| Error::<T>::BoundedVecError)?;


            TableOwners::<T>::insert(&table.ident, ensure_signed(origin.clone())?);

            Ok(table)
        })
        .collect::<Result<Vec<_>, DispatchError>>()?
        .try_into()
        .expect("iterator should still have < MAX_TABLES_PER_SCHEMA elements");

            Self::deposit_event(Event::<T>::SchemaUpdated(owner, tables_with_meta_columns));
            Ok(())
        }
    }

    /// Inject a submitter varchar column to a CreateTableBuilder
    pub fn inject_submitter_column(mut table: CreateTableBuilder) -> CreateTableBuilder {
        let submitter_col = ColumnDef {
            name: Ident::new("submitter"),
            data_type: DataType::Varchar(None), // or DataType::Text if needed
            collation: None,
            options: vec![ColumnOptionDef {
                name: None,
                option: ColumnOption::NotNull,
            }],
        };

        table.columns.push(submitter_col);
        table
    }
}
