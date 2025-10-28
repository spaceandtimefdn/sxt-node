//! The Tables pallet provides functionality related to interacting with tables on
//! the SXT Chain. It provides support for creating, dropping, and editing tables and
//! schemas.
#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

extern crate alloc;
extern crate core;

use alloc::string::String;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod runtime_api;

pub mod weights;
pub use weights::*;

#[allow(clippy::manual_inspect)]
#[frame_support::pallet]
pub mod pallet {
    use alloc::boxed::Box;
    use core::str::from_utf8;

    use codec::alloc::borrow::ToOwned;
    use commitment_sql::CreateTableAndCommitmentMetadata;
    use frame_support::dispatch::DispatchResult;
    use frame_support::pallet_prelude::{StorageDoubleMap, ValueQuery, *};
    use frame_support::Blake2_128Concat;
    use frame_system::pallet_prelude::*;
    use frame_system::RawOrigin;
    use proof_of_sql_commitment_map::{
        CommitmentSchemeFlags,
        TableCommitmentBytesPerCommitmentScheme,
    };
    use scale_info::prelude::vec;
    use sp_core::crypto::Ss58Codec;
    use sp_core::U256;
    use sp_runtime::Vec;
    use sxt_core::permissions::*;
    use sxt_core::tables::{
        create_statement_to_sqlparser,
        create_statement_to_sqlparser_remove_with,
        extract_create_schema_namespace,
        extract_schema_uuid,
        generate_column_uuid_list,
        generate_namespace_uuid,
        generate_table_uuid,
        sqlparser_to_create_statement,
        table_schema_from_create_statement,
        update_uuid_in_create_table_statement,
        uuids_from_sqlparser,
        ColumnUuidList,
        CommitmentBytes,
        CommitmentScheme,
        CreateStatement,
        GetTableSchemaError,
        IdentifierList,
        InsertQuorumSize,
        SnapshotUrl,
        Source,
        SourceAndMode,
        TableIdentifier,
        TableName,
        TableNamespace,
        TableSchema,
        TableType,
        TableUuid,
        TableVersion,
        TryNormalize,
        UpdateUuidError,
    };
    use sxt_core::ByteString;

    use super::*;
    use crate::Event::{NamespaceUuidUpdated, TableUuidUpdated};

    /// A wrapper type that contains all the information needed to create a table
    /// with or without a historical commitment and associated snapshot
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
        /// A runtime event binding to the runtime
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// The weight info to be used for calls
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
        /// The UUID for a given namespace has been updated
        NamespaceUuidUpdated {
            /// The previous UUID of the namespace
            old_uuid: TableUuid,
            /// The new UUID of the namespace
            new_uuid: TableUuid,
            /// The namespace version that was updated
            version: TableVersion,
            /// The namespace that was updated
            namespace: TableNamespace,
        },
        /// The UUID for a given table has been updated
        TableUuidUpdated {
            /// The previous UUID of the table
            old_uuid: TableUuid,
            /// The new UUID of the table
            new_uuid: TableUuid,
            /// The table version that was updated
            version: TableVersion,
            /// The table identifier that was updated
            table: TableIdentifier,
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

        /// A table had its quorum requirements updated
        QuorumUpdated {
            /// The table that was updated
            table: TableIdentifier,
            /// The old quorum settings
            old_quorum: Option<InsertQuorumSize>,
            /// The new quorum Settings
            new_quorum: InsertQuorumSize,
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
    /// Only used for community tables.
    #[pallet::storage]
    pub type TableOwners<T: Config> =
        StorageMap<_, Blake2_128Concat, TableIdentifier, Option<T::AccountId>, ValueQuery>;

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

        /// The provided namespace was invalid for the table
        InvalidNamespace,

        /// The provided name was invalid for the table
        InvalidTableName,

        /// The table used a reserved Column Name
        ReservedColumnName,
    }

    /// The implementation for the pallet extrinsics
    #[pallet::call]
    impl<T: Config> Pallet<T>
    where
        T::AccountId: Ss58Codec,
    {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::update_tables())]
        /// Create table from a provided list with identifiers and DDLs
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
                    )?;

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
            // If all the tables being submitted are public or community, then we only need the transaction to
            // be signed, not specially permissioned
            let is_public =
                table_type == TableType::PublicPermissionless || table_type == TableType::Community;

            let is_privileged = pallet_permissions::Pallet::<T>::ensure_root_or_permissioned(
                origin.clone(),
                &PermissionLevel::TablesPallet(TablesPalletPermission::EditSchema),
            );

            let maybe_owner = ensure_signed(origin.clone());
            match (is_public, is_privileged, maybe_owner) {
                (true, Err(_), Ok(owner)) => {
                    // A user without elevated permissions is trying to create a namespace for a public table type
                    // Ensure a safe name
                    ensure_safe_namespace::<T>(&owner, &schema_name)?;
                }
                (_, Ok(_), _) => {
                    // A user with elevated permission is trying to create a new namespace
                    // No Checks
                }
                _ => {
                    Err(pallet_permissions::Error::<T>::InsufficientPermissions)?;
                }
            }

            let normalized_schema_name = schema_name
                .try_normalize()
                .map_err(|_| Error::<T>::SchemaNameParseError)?;
            let raw_sql =
                from_utf8(&create_statement).map_err(|_| Error::<T>::CreateStatementParseError)?;

            let block_number = <frame_system::Pallet<T>>::block_number();

            let schema_name_s =
                from_utf8(&normalized_schema_name).map_err(|_| Error::<T>::SchemaNameParseError)?;

            ensure!(
                schema_name_s
                    == extract_create_schema_namespace(raw_sql)
                        .ok_or(Error::<T>::CreateStatementParseError)?,
                Error::<T>::InvalidNamespace
            );
            let namespace_uuid = match extract_schema_uuid(raw_sql) {
                Some(uuid) => TableUuid::try_from(uuid.as_bytes().to_vec())
                    .map_err(|_| Error::<T>::TableUUIDError)?,
                None => generate_namespace_uuid(block_number.into(), schema_name_s)
                    .ok_or(Error::<T>::UUIDGenerationError)?,
            };

            Self::insert_namespace_uuid(normalized_schema_name, version, namespace_uuid.clone())?;

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

        /// Update the UUID for the specificed namespace and version to the provided UUID
        #[pallet::call_index(7)]
        #[pallet::weight(<T as Config>::WeightInfo::update_namespace_uuid())]
        pub fn update_namespace_uuid(
            origin: OriginFor<T>,
            namespace: TableNamespace,
            version: TableVersion,
            new_uuid: TableUuid,
        ) -> DispatchResult {
            let _ = pallet_permissions::Pallet::<T>::ensure_root_or_permissioned(
                origin.clone(),
                &PermissionLevel::TablesPallet(TablesPalletPermission::EditUuid),
            )?;

            let maybe_uuid = NamespaceVersions::<T>::get(&namespace, version);
            NamespaceVersions::<T>::set(&namespace, version, new_uuid.clone());

            Self::deposit_event(NamespaceUuidUpdated {
                old_uuid: maybe_uuid,
                new_uuid,
                version,
                namespace,
            });

            Ok(())
        }

        /// Update the UUID for the specified table and version to the provided UUID
        #[pallet::call_index(8)]
        #[pallet::weight(<T as Config>::WeightInfo::update_table_uuid())]
        pub fn update_table_uuid(
            origin: OriginFor<T>,
            table: TableIdentifier,
            version: TableVersion,
            new_uuid: TableUuid,
        ) -> DispatchResult {
            let _ = pallet_permissions::Pallet::<T>::ensure_root_or_permissioned(
                origin.clone(),
                &PermissionLevel::TablesPallet(TablesPalletPermission::EditUuid),
            )?;

            let maybe_uuid = TableVersions::<T>::get(&table, version);
            TableVersions::<T>::set(&table, version, new_uuid.clone());

            if let Some(old_create_statement) = Schemas::<T>::get(&table.namespace, &table.name) {
                let updated = update_uuid_in_create_table_statement(
                    new_uuid.clone(),
                    ColumnUuidList::default(),
                    old_create_statement,
                )
                .map_err(map_uuid_error::<T>)?;
                Schemas::<T>::set(&table.namespace, &table.name, Some(updated));
            }

            Self::deposit_event(TableUuidUpdated {
                old_uuid: maybe_uuid,
                new_uuid,
                version,
                table,
            });

            Ok(())
        }

        /// Transaction for updating the quorum of a particular table
        #[pallet::call_index(9)]
        #[pallet::weight(<T as Config>::WeightInfo::update_table_quorum())]
        pub fn update_table_quorum(
            origin: OriginFor<T>,
            table: TableIdentifier,
            new_quorum: InsertQuorumSize,
        ) -> DispatchResult {
            let _ = pallet_permissions::Pallet::<T>::ensure_root_or_permissioned(
                origin.clone(),
                &PermissionLevel::TablesPallet(TablesPalletPermission::EditSchema),
            )?;

            let old_quorum = Self::inner_update_table_quorum(&table, new_quorum);

            Self::deposit_event(Event::QuorumUpdated {
                table,
                new_quorum,
                old_quorum,
            });
            Ok(())
        }

        /// Transaction for updating the quorum of all tables in the provided Schema
        #[pallet::call_index(10)]
        #[pallet::weight(<T as Config>::WeightInfo::update_table_quorum() * 10)]
        pub fn update_schema_quorum(
            origin: OriginFor<T>,
            schema_name: sxt_core::tables::TableNamespace,
            new_quorum: InsertQuorumSize,
        ) -> DispatchResult {
            let _ = pallet_permissions::Pallet::<T>::ensure_root_or_permissioned(
                origin.clone(),
                &PermissionLevel::TablesPallet(TablesPalletPermission::EditSchema),
            )?;

            let tables: Vec<TableIdentifier> = Schemas::<T>::iter_key_prefix(&schema_name)
                .map(|name: TableName| TableIdentifier {
                    name,
                    namespace: schema_name.clone(),
                })
                .collect();

            tables.iter().for_each(|table: &TableIdentifier| {
                let old_quorum = Self::inner_update_table_quorum(table, new_quorum);

                Self::deposit_event(Event::QuorumUpdated {
                    table: table.clone(),
                    new_quorum,
                    old_quorum,
                });
            });

            Ok(())
        }
    }

    fn map_uuid_error<T: Config>(error: UpdateUuidError) -> DispatchError {
        match error {
            UpdateUuidError::InvalidUuid => Error::<T>::TableUUIDError.into(),
            UpdateUuidError::ParseError { .. } => Error::<T>::CreateStatementParseError.into(),
        }
    }

    impl<T: Config> Pallet<T>
    where
        T::AccountId: Ss58Codec,
    {
        /// Remove commits based on identifier
        pub fn remove_commits(ident: TableIdentifier) {
            for (k1, k2, _) in pallet_commitments::CommitmentStorageMap::<T>::iter() {
                if k1 == ident {
                    pallet_commitments::CommitmentStorageMap::<T>::remove(&ident, k2);
                }
            }
        }

        /// Update the quorum entry for the provided table, returning the old quorum, if present
        pub fn inner_update_table_quorum(
            id: &TableIdentifier,
            new_quorum: InsertQuorumSize,
        ) -> Option<InsertQuorumSize> {
            let old_quorum = TableInsertQuorums::<T>::get(id);

            TableInsertQuorums::<T>::set(id, new_quorum);

            Some(old_quorum)
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

        /// Update the schema and commitment for a table and source and mode combo
        pub fn insert_schema(
            ident: TableIdentifier,
            stmnt: CreateStatement,
            table_type: TableType,
            source: Source,
        ) -> DispatchResult {
            let mut identifiers = Identifiers::<T>::get(&table_type);

            identifiers
                .try_push(ident.clone())
                .map_err(|_| Error::<T>::BoundedVecError)?;
            Identifiers::<T>::insert(&table_type, identifiers);

            let TableIdentifier { name, namespace } = ident.clone();
            Schemas::<T>::insert(namespace, name, stmnt.clone());
            let quorum: InsertQuorumSize = table_type.into();

            TableInsertQuorums::<T>::insert(&ident, quorum);
            TableSources::<T>::insert(&ident, source);
            Ok(())
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

            let CreateTableAndCommitmentMetadata {
                table_with_meta_columns,
                ..
            } = pallet_commitments::Pallet::<T>::process_create_table_from_snapshot_and_initiate_commitments(create_table, commit)?;

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
            } = pallet_commitments::Pallet::<T>::process_create_table_and_initiate_commitments_with_dynamic_dory(create_table)?;

            let statement_with_metadata = sqlparser_to_create_statement(table_with_meta_columns)
                .map_err(|_| Error::<T>::CreateStatementParseError)?;

            Snapshots::<T>::insert(ident, snapshot);

            Ok(statement_with_metadata)
        }

        /// Attempts to retrieve the UUID for the provided table from the CreateStatement,
        /// generating UUIDs if that fails.
        pub fn get_or_generate_uuids_for_table(
            raw: CreateStatement,
            identifier: TableIdentifier,
        ) -> Result<(TableUuid, ColumnUuidList), DispatchError> {
            // Try parsing the statement
            if let Ok(parsed) = create_statement_to_sqlparser(raw.clone()) {
                if let Ok((table_uuid, column_uuids)) = uuids_from_sqlparser(parsed) {
                    let has_any = table_uuid != TableUuid::default() || !column_uuids.is_empty();
                    if has_any {
                        return Ok((table_uuid, column_uuids));
                    }
                }
            }

            // Fallback: generate new UUIDs
            let block_number = <frame_system::Pallet<T>>::block_number();

            let namespace = from_utf8(&identifier.namespace)
                .map_err(|_| DispatchError::Other("Invalid UTF-8 namespace"))?;

            let name = from_utf8(&identifier.name)
                .map_err(|_| DispatchError::Other("Invalid UTF-8 name"))?;

            let table_uuid = generate_table_uuid(block_number.into(), namespace, name)
                .ok_or(Error::<T>::UUIDGenerationError)?;
            let column_uuids = generate_column_uuid_list(raw)?;

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

            // Remove the snapshots entry.
            if Snapshots::<T>::contains_key(&ident) {
                Snapshots::<T>::remove(&ident);
            }

            // Remove the table sources entry.
            if TableSources::<T>::contains_key(&ident) {
                TableSources::<T>::remove(&ident);
            }

            // Remove the table owner entry.
            if TableOwners::<T>::contains_key(&ident) {
                TableOwners::<T>::remove(&ident);
            }

            // Remove commits
            Self::remove_commits(ident.clone());

            Ok(())
        }

        /// Create a table. Exactly the same as the extrinsic
        pub fn create_tables_inner(
            origin: OriginFor<T>,
            tables: UpdateTableList,
        ) -> DispatchResult {
            // If all the tables being submitted are public or community, then we only need the transaction to
            // be signed, not specially permissioned
            let all_public = tables.iter().all(|table| {
                table.table_type == TableType::PublicPermissionless
                    || table.table_type == TableType::Community
            });
            let is_privileged = pallet_permissions::Pallet::<T>::ensure_root_or_permissioned(
                origin.clone(),
                &PermissionLevel::TablesPallet(TablesPalletPermission::EditSchema),
            );

            let owner = match (all_public, is_privileged, ensure_signed(origin.clone())) {
                (true, Err(_), Ok(o)) => {
                    // A user without elevated permissions is trying to create public tables
                    // Ensure safe names
                    for t in tables.iter() {
                        ensure_safe_namespace::<T>(&o, &t.ident.namespace)?;
                    }
                    Some(o)
                }
                (_, Ok(_), Ok(o)) => {
                    // A user with elevated permission is trying to create a new namespace
                    // No Checks, but return the account for processing
                    Some(o)
                }
                (_, Ok(_), Err(_)) => {
                    // Elevated permissions, but no account. Likely a sudo call or a unit test
                    None
                }
                _ => {
                    return Err(pallet_permissions::Error::<T>::InsufficientPermissions.into());
                }
            };

            let tables_with_meta_columns = tables
                .into_iter()
                .map(|mut table| {
                    let is_permissionless = table.table_type == TableType::PublicPermissionless;

                    table.ident = table
                        .ident
                        .try_normalize()
                        .map_err(|_| Error::<T>::InvalidNamespace)?;

                    ensure!(
                        NamespaceVersions::<T>::iter_key_prefix(&table.ident.namespace)
                            .next()
                            .is_some(),
                        DispatchError::Other("Namespace does not exist.")
                    );

                    // Generate or extract UUIDs
                    let (table_uuid, column_uuids) =
                        pallet::Pallet::<T>::get_or_generate_uuids_for_table(
                            table.create_statement.clone(),
                            table.ident.clone(),
                        )
                        .map_err(|_| Error::<T>::UUIDGenerationError)?;

                    // Update the create statement to add the UUIDs to the WITH clause
                    let updated_create_statement = update_uuid_in_create_table_statement(
                        table_uuid.clone(),
                        column_uuids.clone(),
                        table.create_statement.clone(),
                    )
                    .map_err(map_uuid_error::<T>)?;

                    Self::insert_table_uuid(table.ident.clone(), table_uuid, column_uuids)?;
                    Self::insert_schema(
                        table.ident.clone(),
                        updated_create_statement.clone(),
                        table.table_type.clone(),
                        table.source.clone(),
                    )?;

                    // Parse and remove WITH clause
                    let (mut create_table, with_options) =
                        create_statement_to_sqlparser_remove_with(updated_create_statement)
                            .map_err(|_| Error::<T>::CreateStatementParseError)?;

                    for identifier in create_table.name.0.iter_mut() {
                        identifier.value = identifier.value.to_uppercase();
                    }

                    // Ensure the parsed table identifier matches the provided one
                    match (
                        create_table.name.0.as_slice(),
                        from_utf8(&table.ident.namespace),
                        from_utf8(&table.ident.name),
                    ) {
                        ([parsed_namespace, parsed_name], Ok(param_namespace), Ok(param_name))
                            if parsed_namespace.value == param_namespace
                                && parsed_name.value == param_name =>
                        {
                            Ok(())
                        }
                        _ => Err(Error::<T>::TableIdentifierParsingError),
                    }?;

                    // Inject submitter column if this is a permissionless table
                    if is_permissionless {
                        if sxt_core::tables::has_submitter_column(&create_table) {
                            return Err(Error::<T>::ReservedColumnName.into());
                        }
                        create_table = sxt_core::tables::inject_submitter_column(create_table);
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
                        CommitmentCreationCmd::FromSnapshot(
                            ref snapshot_url,
                            ref per_commitment_scheme,
                        ) => Err(DispatchError::Other(
                            "Snapshot commitments are deprecated in this extrinsic",
                        ))?,
                    };

                    // Reconstruct final DDL statement
                    let statement_with_metadata =
                        sqlparser_to_create_statement(table_with_meta_columns)
                            .map_err(|_| Error::<T>::CreateStatementParseError)?;

                    let statement_with_metadata = from_utf8(&statement_with_metadata)
                        .map_err(|_| Error::<T>::UtfConversionError)?;

                    let reconstructed = match with_options {
                        Some(opts) => {
                            let mut base = statement_with_metadata.trim_end_matches(';').to_owned();
                            base.push(' ');
                            base.push_str(
                                from_utf8(&opts).map_err(|_| Error::<T>::UtfConversionError)?,
                            );
                            base.push(';');
                            base
                        }
                        None => {
                            let mut base = statement_with_metadata.trim_end_matches(';').to_owned();
                            base.push(';');
                            base
                        }
                    };

                    table.create_statement =
                        CreateStatement::try_from(reconstructed.as_bytes().to_vec())
                            .map_err(|_| Error::<T>::BoundedVecError)?;

                    // Grant permission to the table creator to submit data and grant others permission to
                    // submit data
                    if let Some(owner) = owner.clone() {
                        let table_permission = PermissionLevel::IndexingPallet(
                            IndexingPalletPermission::SubmitDataForPrivilegedQuorum(
                                table.ident.clone(),
                            ),
                        );

                        let _ = pallet_permissions::Pallet::<T>::add_proxy_permission(
                            RawOrigin::Root.into(),
                            owner.clone(),
                            table_permission.clone(),
                        );

                        let _ = pallet_permissions::Pallet::<T>::add_proxy_permission(
                            RawOrigin::Root.into(),
                            owner.clone(),
                            PermissionLevel::EditSpecificPermission(Box::new(table_permission)),
                        );
                    }

                    TableOwners::<T>::insert(&table.ident, owner.clone());

                    Ok(table)
                })
                .collect::<Result<Vec<_>, DispatchError>>()?
                .try_into()
                .expect("iterator should still have < MAX_TABLES_PER_SCHEMA elements");

            Self::deposit_event(Event::<T>::SchemaUpdated(owner, tables_with_meta_columns));
            Ok(())
        }

        /// Returns the schema for the given table identifier.
        pub fn table_schema(
            table_identifier: TableIdentifier,
        ) -> Result<TableSchema, GetTableSchemaError> {
            let create_statement = Self::schemas(table_identifier.namespace, table_identifier.name)
                .ok_or(GetTableSchemaError::NoSuchTable)?;

            let table_schema = table_schema_from_create_statement(create_statement)?;

            Ok(table_schema)
        }
    }

    // Check if a public table has a safe name. Accepts an account ID and a table Identifier and determines if the table identifier is
    // valid for the provided table owner.
    pub(crate) fn ensure_safe_namespace<T: Config>(
        who: &T::AccountId,
        namespace: &ByteString,
    ) -> Result<(), DispatchError>
    where
        T::AccountId: Ss58Codec,
    {
        // Get the namespace as a string
        let val = from_utf8(namespace).map_err(|_| Error::<T>::UtfConversionError)?;

        // Substrate addresses are 32 bytes, which, in hex string format, exceeds our character
        // limit. Because of this, substrate addresses must use their SS58 format, but to improve user
        // experience, Ethereum use their hex string format. This distinction requires use to check
        // which kind of account we're handling.

        // Check if this address has the ten leading 0's
        let is_ethereum = is_ethereum_address::<T>(who.clone());
        let is_valid_eth_namespace = is_valid_eth_user_namespace(val, who.clone().encode());
        let is_valid_ss58 = is_valid_ss58_user_namespace(val, Ss58Codec::to_ss58check(who));

        if is_ethereum && is_valid_eth_namespace {
            return Ok(());
        }

        if is_valid_ss58 {
            return Ok(());
        }

        Err(Error::<T>::InvalidNamespace.into())
    }

    // Returns true if the provided bytes coule be a valid ethereum address
    pub(crate) fn is_ethereum_address<T: Config>(account: T::AccountId) -> bool {
        let eth_bytes = account.encode();
        // Remove the leiading 0s from the bytes before comparing
        let trimmed_bytes: Vec<u8> = eth_bytes
            .into_iter()
            .skip_while(|&byte| byte == 0)
            .collect(); // U256 with 10 leading 0's
                        // Eth addresses are 20 bytes, so this is the maximum possible value of an eth
                        // address in our system

        let address = U256::from_little_endian(&trimmed_bytes);

        let eth_max_val =
            U256::from_str_radix("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF", 16).unwrap();

        address < eth_max_val
    }

    /// Returns true if the namespace is valid for the provided eth address bytes
    fn is_valid_eth_user_namespace(namespace: &str, eth_bytes: Vec<u8>) -> bool {
        // Remove the leading 0s from the bytes before comparing
        let trimmed_bytes: Vec<u8> = eth_bytes
            .into_iter()
            .skip_while(|&byte| byte == 0)
            .collect();

        namespace
            .to_uppercase()
            .ends_with(&hex::encode(trimmed_bytes).to_uppercase())
    }

    /// Returns true if the namespace is valid for the provided SS58 encoded address string
    fn is_valid_ss58_user_namespace(namespace: &str, ss58_user: String) -> bool {
        // The namespace of user created tales must end with the users hexadecimal address
        namespace
            .to_uppercase()
            .ends_with(&ss58_user.to_uppercase())
    }
}
