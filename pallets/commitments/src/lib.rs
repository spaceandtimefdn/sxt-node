#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod test_initiate_precomputed_commitments;

#[cfg(test)]
mod test_create_table_generic;

#[cfg(test)]
mod test_create_table;

#[cfg(test)]
mod test_create_table_from_snapshot;

#[cfg(test)]
mod test_insert;

#[cfg(test)]
mod test_table_commitments;

mod error_conversions;

pub mod runtime_api;
pub use pallet::*;

#[allow(clippy::manual_inspect)]
#[frame_support::pallet]
pub mod pallet {
    use alloc::vec::Vec;
    use alloc::{str, vec};

    use commitment_sql::{
        process_create_table,
        process_create_table_from_snapshot,
        CreateTableAndCommitmentMetadata,
        InsertAndCommitmentMetadata,
    };
    use frame_support::pallet_prelude::*;
    use native_api::NativeApi;
    use on_chain_table::OnChainTable;
    use proof_of_sql_commitment_map::generic_over_commitment::ConcreteType;
    use proof_of_sql_commitment_map::{
        AnyCommitmentScheme,
        CommitmentMap,
        CommitmentScheme,
        CommitmentSchemeFlags,
        CommitmentStorageMapHandler,
        KeyExistsError,
        TableCommitmentBytes,
        TableCommitmentBytesPerCommitmentScheme,
        TableCommitmentBytesPerCommitmentSchemePassBy,
    };
    use proof_of_sql_static_setups::baked::PUBLIC_SETUPS;
    use sqlparser::ast::helpers::stmt_create_table::CreateTableBuilder;
    use sxt_core::tables::TableIdentifier;

    use super::*;

    /// Commitment pallet, providing methods for pallet calls
    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// The commitment pallet's configuration trait.
    #[pallet::config]
    pub trait Config: frame_system::Config {}

    /// Mapping of tables to their current commitments, stored on chain.
    #[pallet::storage]
    #[pallet::getter(fn table_commitment)]
    pub type CommitmentStorageMap<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        TableIdentifier,
        Blake2_128Concat,
        CommitmentScheme,
        TableCommitmentBytes,
    >;

    /// Default schemes used when committing to new tables.
    #[pallet::storage]
    pub type DefaultCommitmentSchemes<T: Config> = StorageValue<_, CommitmentSchemeFlags>;

    /// Genesis configuration struct for the commitments pallet.
    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        default_commitment_schemes: CommitmentSchemeFlags,
        _marker: PhantomData<T>,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            let default_commitment_schemes = CommitmentSchemeFlags {
                hyper_kzg: true,
                dynamic_dory: true,
            };

            GenesisConfig {
                default_commitment_schemes,
                _marker: PhantomData,
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            DefaultCommitmentSchemes::<T>::put(self.default_commitment_schemes);
        }
    }

    /// The errors that can occur within this pallet.
    #[pallet::error]
    #[derive(PartialEq, Eq)]
    pub enum Error<T> {
        /// Proof-of-sql commitment has too many columns.
        CommitmentWithTooManyColumns,
        /// Failed to serialize proof-of-sql commitment.
        SerializeCommitment,
        /// Failed to deserialize proof-of-sql commitment.
        DeserializeCommitment,
        /// Failed to serialize `OnChainTable`.
        SerializeInsertData,
        /// Failed to deserialize `OnChainTable`.
        DeserializeInsertData,
        /// Snapshot commitments don't match table definition.
        InappropriateSnapshotCommitments,
        /// Table must have at least one column.
        CreateTableWithNoColumns,
        /// Table ref has unexpected number of identifiers.
        CreateTableWithInvalidTableIdentifierCount,
        /// Table has duplicate identifiers.
        CreateTableWithDuplicateIdentifiers,
        /// Table uses reserved metadata prefix.
        CreateTableWithReservedMetadataPrefix,
        /// Decimal/numeric columns should have constrained precision and scale.
        DecimalColumnWithoutPrecision,
        /// Decimal/numeric columns should have precision between 1 and 75.
        DecimalColumnWithInvalidPrecision,
        /// Decimal/numeric columns should have scale between 0 and 127.
        DecimalColumnWithInvalidScale,
        /// Column type supported but not type parameter.
        SupportedColumnWithUnsupportedParameter,
        /// Column type not supported.
        ColumnWithUnsupportedDataType,
        /// Column should be NOT NULL.
        ColumnWithoutNotNull,
        /// Column option not supported.
        ColumnWithUnsupportedOption,
        /// Failed to serialize proof-of-sql commitment in native interface.
        NativeSerializeCommitment,
        /// Failed to deserialize proof-of-sql commitment in native interface.
        NativeDeserializeCommitment,
        /// Failed to serialize `OnChainTable` in native interface.
        NativeSerializeInsertData,
        /// Failed to deserialize `OnChainTable` in native interface.
        NativeDeserializeInsertData,
        /// Existing commitments of different schemes don't agree on table range.
        ExistingCommitmentsRangeMismatch,
        /// Existing commitments of different schemes don't agree on column order.
        ExistingCommitmentsColumnOrderMismatch,
        /// Cannot update table with no existing commitments.
        NoExistingCommitments,
        /// Insert data contains values out of bounds of scalar field.
        InsertDataOutOfBounds,
        /// Insert data does not match existing commitments.
        InsertDataDoesntMatchExistingCommitments,
        /// Table identifier already exists in commitment storage.
        TableAlreadyExists,
    }

    impl<T: Config> Pallet<T> {
        /// Processes the table definition and initiates commitments for it in storage.
        ///
        /// Returns the original table definition with additional commitment metadata columns.
        pub fn process_create_table_and_initiate_commitments(
            create_table: CreateTableBuilder,
        ) -> Result<CreateTableAndCommitmentMetadata, Error<T>> {
            let schemes = DefaultCommitmentSchemes::<T>::get()
                .expect("default commitment schemes will exist due to genesis config");

            Self::process_create_table_and_initiate_commitments_with_scheme(create_table, schemes)
        }

        /// TODO: docs
        pub fn process_create_table_and_initiate_commitments_with_dynamic_dory(
            create_table: CreateTableBuilder,
        ) -> Result<CreateTableAndCommitmentMetadata, Error<T>> {
            let scheme = CommitmentSchemeFlags {
                dynamic_dory: true,
                hyper_kzg: false,
            };

            Self::process_create_table_and_initiate_commitments_with_scheme(create_table, scheme)
        }

        /// TODO: docs
        pub fn process_create_table_and_initiate_commitments_with_scheme(
            create_table: CreateTableBuilder,
            scheme: CommitmentSchemeFlags,
        ) -> Result<CreateTableAndCommitmentMetadata, Error<T>> {
            let (create_table_and_commitment_metadata, empty_commitments) =
                process_create_table(create_table, *PUBLIC_SETUPS, &scheme)?;

            let mut handler = CommitmentStorageMapHandler::<CommitmentStorageMap<T>>::new();

            let table_identifier = TableIdentifier::try_from(
                &create_table_and_commitment_metadata
                    .table_with_meta_columns
                    .name,
            )
            .expect("Create table identifier already validated by process_create_table");

            let empty_commitments_bytes = empty_commitments.try_into()?;

            handler.create_commitments(table_identifier, empty_commitments_bytes)?;

            Ok(create_table_and_commitment_metadata)
        }

        /// Processes the table definition and stores its snapshot commitments.
        ///
        /// Returns the original table definition with additional commitment metadata columns.
        pub fn process_create_table_from_snapshot_and_initiate_commitments(
            create_table: CreateTableBuilder,
            snapshot_commitment_bytes: TableCommitmentBytesPerCommitmentScheme,
        ) -> Result<CreateTableAndCommitmentMetadata, Error<T>> {
            let snapshot_commitments = snapshot_commitment_bytes
                .try_into()
                .map_err(|_| Error::DeserializeCommitment)?;

            let (create_table_and_commitment_metadata, snapshot_commitments) =
                process_create_table_from_snapshot(
                    create_table,
                    *PUBLIC_SETUPS,
                    snapshot_commitments,
                )?;

            let snapshot_commitment_bytes = snapshot_commitments.try_into()?;

            let mut handler = CommitmentStorageMapHandler::<CommitmentStorageMap<T>>::new();

            let table_identifier = TableIdentifier::try_from(
                &create_table_and_commitment_metadata
                    .table_with_meta_columns
                    .name,
            )
            .expect(
                "Create table identifier already validated by process_create_table_from_snapshot",
            );

            handler.create_commitments(table_identifier, snapshot_commitment_bytes)?;

            Ok(create_table_and_commitment_metadata)
        }

        /// Initiates the provided table with the provided commitments in storage.
        #[deprecated(
            note = "for historical load, use process_create_table_from_snapshot_and_initiate_commitments"
        )]
        pub fn initiate_precomputed_commitments(
            table: TableIdentifier,
            commitments: TableCommitmentBytesPerCommitmentScheme,
        ) -> Result<(), KeyExistsError<TableIdentifier>> {
            let mut handler = CommitmentStorageMapHandler::<CommitmentStorageMap<T>>::new();

            handler.create_commitments(table, commitments)
        }

        /// Processes the insert and updates commitments for the table in storage.
        ///
        /// Returns the original insert with additional commitment metadata columns.
        pub fn process_insert_and_update_commitments<I: NativeApi>(
            table: TableIdentifier,
            insert_data: OnChainTable,
        ) -> Result<InsertAndCommitmentMetadata, Error<T>> {
            let mut handler = CommitmentStorageMapHandler::<CommitmentStorageMap<T>>::new();

            let previous_commitments = TableCommitmentBytesPerCommitmentSchemePassBy {
                data: handler.get_commitments(&table),
            };

            let table_bytes = insert_data.try_into()?;

            let (insert_with_meta_columns_bytes, commitments_bytes) =
                I::process_insert(table.clone(), table_bytes, previous_commitments)?;

            let commitments_bytes = commitments_bytes.data;

            handler
                 .update_commitments(table, commitments_bytes)
                 .expect("process_insert guarantees to update the same commitment schemes that were provided to it");

            let insert_with_meta_columns = insert_with_meta_columns_bytes
                .try_into()
                .map_err(|_| Error::DeserializeInsertData)?;

            Ok(InsertAndCommitmentMetadata {
                insert_with_meta_columns,
                meta_table_inserts: vec![],
            })
        }
    }

    /// Return type for some APIs, a list of table commitments for any scheme.
    pub type AnyTableCommitments = AnyCommitmentScheme<ConcreteType<Vec<TableCommitmentBytes>>>;

    impl<T> Pallet<T>
    where
        T: Config,
    {
        /// Returns the table commitments for the provided tables if and only if all of them exist.
        pub fn table_commitments<'t>(
            table_identifiers: impl IntoIterator<Item = &'t TableIdentifier>,
            commitment_scheme: CommitmentScheme,
        ) -> Option<Vec<TableCommitmentBytes>> {
            table_identifiers
                .into_iter()
                .map(|table_identifier| Self::table_commitment(table_identifier, commitment_scheme))
                .collect()
        }

        /// Returns the table commitments for the provided tables for the first scheme that covers
        /// all of them.
        ///
        /// Returns `None` if no scheme has complete coverage of the provided tables.
        pub fn table_commitments_any_scheme<'t>(
            table_identifiers: impl IntoIterator<Item = &'t TableIdentifier> + Copy,
        ) -> Option<AnyTableCommitments> {
            CommitmentSchemeFlags::all()
                .into_iter()
                .find_map(|commitment_scheme| {
                    Self::table_commitments(table_identifiers, commitment_scheme).map(
                        |table_commitments| commitment_scheme.into_any_concrete(table_commitments),
                    )
                })
        }
    }
}
