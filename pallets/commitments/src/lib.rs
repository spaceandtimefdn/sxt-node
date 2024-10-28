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

mod public_setups;

mod error_conversions;

pub use pallet::*;

#[allow(clippy::manual_inspect)]
#[frame_support::pallet]
pub mod pallet {
    use alloc::str;

    use commitment_sql::{
        process_create_table,
        process_create_table_from_snapshot,
        process_insert,
        CreateTableAndCommitmentMetadata,
        InsertAndCommitmentMetadata,
    };
    use frame_support::pallet_prelude::*;
    use on_chain_table::OnChainTable;
    use proof_of_sql_commitment_map::{
        CommitmentMap,
        CommitmentScheme,
        CommitmentSchemeFlags,
        CommitmentStorageMapHandler,
        KeyExistsError,
        TableCommitmentBytes,
        TableCommitmentBytesPerCommitmentScheme,
    };
    use public_setups::PUBLIC_SETUPS;
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
                ipa: false,
                dory: true,
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
        /// Snapshot commitments don't match table definition.
        InappropriateSnapshotCommitments,
        /// Table must have at least one column.
        CreateTableWithNoColumns,
        /// Table has invalid identifier.
        CreateTableWithInvalidIdentifier,
        /// Table has duplicate identifiers.
        CreateTableWithDuplicateIdentifiers,
        /// Table uses reserved metadata prefix.
        CreateTableWithReservedMetadataPrefix,
        /// Timestamp column precision should be 0, 3, or 6.
        TimestampColumnWithInvalidPrecision,
        /// Decimal/numeric columns should have constrained precision and scale.
        DecimalColumnWithoutPrecision,
        /// Decimal/numeric columns should have precision between 1 and 75.
        DecimalColumnWithInvalidPrecision,
        /// Decimal/numeric columns should have scale between 0 and 127.
        DecimalColumnWithInvalidScale,
        /// Column type not supported.
        ColumnWithUnsupportedDataType,
        /// Column should be NOT NULL.
        ColumnWithoutNotNull,
        /// Column option not supported.
        ColumnWithUnsupportedOption,
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

            let (create_table_and_commitment_metadata, empty_commitments) =
                process_create_table(create_table, *PUBLIC_SETUPS, &schemes)?;

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
        pub fn process_insert_and_update_commitments(
            table: TableIdentifier,
            insert_data: OnChainTable,
        ) -> Result<InsertAndCommitmentMetadata, Error<T>> {
            let mut handler = CommitmentStorageMapHandler::<CommitmentStorageMap<T>>::new();

            let previous_commitments = handler
                .get_commitments(&table)
                .try_into()
                .map_err(|_| Error::DeserializeCommitment)?;

            let (insert_and_commitment_metadata, commitments) =
                process_insert(&table, insert_data, previous_commitments, *PUBLIC_SETUPS)?;

            let commitments_bytes = commitments.try_into()?;

            handler
                .update_commitments(table, commitments_bytes)
                .expect("process_insert guarantees to update the same commitment schemes that were provided to it");

            Ok(insert_and_commitment_metadata)
        }
    }
}
