//! The Indexing Pallet
//! The indexing pallet implements the functionality needed to allow indexers to submit data
//! via the `submit_data` extrinsic. Once data is submitted, it contains logic for checking if
//! we have enough submissions to reach a quorum, and if we do, it will finalize the data and
//! emit an event stating that the batch id has been decided on. The event also contains the
//! final data for the decision.

// We make sure this pallet uses `no_std` for compiling to Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
extern crate core;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;
pub mod weights;
// Do not remove this or the same attribute for the pallet
// The cargo doc command will fail because of a bug even though the code is working properly
pub use pallet::*;
pub use sxt_core::indexing::*;
pub use weights::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

mod error_conversions;

/// Native wrapper around the indexing pallet.
pub mod native_pallet;

pub mod migrations;

#[allow(clippy::manual_inspect)]
#[frame_support::pallet]
pub mod pallet {
    use alloc::collections::{BTreeMap, BTreeSet};
    use alloc::string::String;
    use alloc::vec::Vec;

    use codec::Decode;
    use commitment_sql::InsertAndCommitmentMetadata;
    use frame_support::dispatch::RawOrigin;
    use frame_support::pallet_prelude::*;
    use frame_support::{Blake2_128, Blake2_128Concat};
    use frame_system::pallet_prelude::*;
    use hex::FromHex;
    use itertools::{Either, Itertools};
    use native_api::NativeApi;
    use on_chain_table::OnChainTable;
    use sp_core::{H256, U256};
    use sp_runtime::traits::{Bounded, Hash, StaticLookup, UniqueSaturatedInto};
    use sp_runtime::{BoundedVec, SaturatedConversion};
    use sxt_core::permissions::{IndexingPalletPermission, PermissionLevel};
    use sxt_core::tables::{
        InsertQuorumSize,
        QuorumScope,
        TableIdentifier,
        TableName,
        TableNamespace,
    };
    use sxt_core::IdentLength;

    use super::*;

    #[pallet::pallet]
    pub struct Pallet<T, I = ()>(_);

    #[pallet::config]
    pub trait Config<I: 'static = ()>:
        frame_system::Config
        + pallet_permissions::Config
        + pallet_commitments::Config
        + pallet_tables::Config
        + pallet_system_tables::Config
    {
        /// Binding for the runtime event, typically provided by an implementation
        /// in runtime/lib.rs
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// The weight info to be used with the extrinsics provided by the pallet
        type WeightInfo: WeightInfo;
    }

    /// Hashed data submissions that have yet to reach quorum.
    #[pallet::storage]
    #[pallet::getter(fn submissions)]
    pub type SubmissionsV1<T: Config<I>, I: 'static = ()> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, BatchId>,
            NMapKey<Blake2_128Concat, QuorumScope>,
            NMapKey<Blake2_128Concat, T::AccountId>,
        ),
        <T as frame_system::Config>::Hash,
    >;

    #[pallet::storage]
    #[pallet::getter(fn final_data)]
    pub type FinalData<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128Concat, BatchId, DataQuorum<T::AccountId, T::Hash>>;

    #[pallet::storage]
    #[pallet::getter(fn block_numbers)]
    pub type BlockNumbers<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128Concat, TableIdentifier, u64>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        /// This event is emitted every time data is submitted by an indexer.
        /// It can be used to verify that the data was successfully processed and received.
        DataSubmitted {
            /// The account id of the submitter
            who: T::AccountId,
            /// The submission that was sent. Only contains the hash of the original data
            submission: DataSubmission<T::Hash>,
        },
        /// This event is emitted when a quorum is reached amongst submissions and the
        /// data is finalized.
        QuorumReached {
            /// The quorum object representing the metadata about the decision
            quorum: DataQuorum<T::AccountId, T::Hash>,
            /// The finalized raw data in postcard serialized OnChainTable bytes
            data: BoundedVec<u8, ConstU32<DATA_MAX_LEN>>,
        },
        /// Emitted when a system meta table should insert new rows due to some on-chain
        /// action
        SystemTableUpdate {
            /// The table that was updated
            table: TableIdentifier,
            /// The postcard serialized OnChainTable bytes for the system table insert
            data: BoundedVec<u8, ConstU32<DATA_MAX_LEN>>,
        },
        /// Emitted any time there's an error while processing a system event
        /// This message can then be handled offline to initiate retries or remediation
        SystemTableError {
            /// The table that had an error
            table: TableIdentifier,
            /// The error received while processing the insert
            error: DispatchError,
            /// The postcard serialized OnChainTable bytes for the system table insert
            data: BoundedVec<u8, ConstU32<DATA_MAX_LEN>>,
        },

        /// A quorum has been decided for any empty block
        QuorumEmptyBlock {
            /// The table identifier
            table: TableIdentifier,
            /// The block number that quorum was reached over
            block_number: u64,
            /// Voters for this quorum
            agreements: BoundedBTreeSet<T::AccountId, ConstU32<MAX_SUBMITTERS>>,
            /// Voters against this quorum
            dissents: BoundedBTreeSet<T::AccountId, ConstU32<MAX_SUBMITTERS>>,
        },
    }

    #[pallet::error]
    pub enum Error<T, I = ()> {
        /// The signer of the transaction is not authorized to submit data
        UnauthorizedSubmitter,
        /// The data submitted doesn't match the schema for the target table
        SchemaMismatch,
        /// No Data was provided with the submission
        NoData,
        /// Invalid BatchId was provided
        InvalidBatch,
        /// The BatchId Provided has already been decided on
        LateBatch,
        /// Invalid Table identifier was supplied
        InvalidTable,
        /// The table could not be deserialized using a Stream Reader
        NativeDeserializationError,
        /// There was no record batch contained in the data
        NativeEmptyRecordBatchError,
        /// Error reading record batch
        NativeBatchReadError,
        /// RecordBatch column has unsupported type
        NativeRecordBatchUnsupportedType,
        /// RecordBatch contains nulls
        NativeRecordBatchContainsNulls,
        /// RecordBatch has invalid timezone
        NativeRecordBatchInvalidTimezone,
        /// RecordBatch has unexpected mismatch between schema and data
        NativeRecordBatchUnexpectedSchemaDataMismatch,
        /// RecordBatch has duplicate identifiers
        NativeRecordBatchDuplicateIdentifiers,
        /// Error serializing the OnChainTable
        NativeSerializationError,
        /// Maximum submissions already reached for this batch id
        MaxSubmittersReached,
        /// Error deserializing the table as an OnChainTable
        TableDeserializationError,
        /// Error deserializing the table as an OnChainTable
        TableSerializationError,
    }

    #[pallet::call]
    impl<T: Config<I>, I: 'static> Pallet<T, I>
    where
        T: pallet_tables::Config,
        I: NativeApi,
    {
        /// This extrinsic provides a transaction that indexers will use to submit
        /// data they've indexed.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config<I>>::WeightInfo::submit_data())]
        pub fn submit_data(
            origin: OriginFor<T>,
            table: TableIdentifier,
            batch_id: BatchId,
            data: RowData,
        ) -> DispatchResult {
            submit_data_inner::<T, I>(origin, table, batch_id, data, None)
        }

        /// Submit a new data batch with an associated block number.
        ///
        /// This extrinsic is used by indexers (e.g., Garfield, Gateway) to submit a chunk of indexed data
        /// to a given table. It includes an explicit `block_number` to represent the highest block covered
        /// by this batch. The submission goes through the quorum process (public or privileged) and is
        /// finalized only if quorum is reached.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config<I>>::WeightInfo::submit_data())]
        pub fn submit_blockchain_data(
            origin: OriginFor<T>,
            table: TableIdentifier,
            batch_id: BatchId,
            data: RowData,
            block_number: u64,
        ) -> DispatchResult {
            submit_data_inner::<T, I>(origin, table, batch_id, data, Some(block_number))
        }
    }

    fn submit_data_inner<T, I>(
        origin: OriginFor<T>,
        table: TableIdentifier,
        batch_id: BatchId,
        data: RowData,
        block_number: Option<u64>,
    ) -> DispatchResult
    where
        T: Config<I>,
        I: NativeApi,
    {
        let who = ensure_signed(origin.clone())?;
        let table_insert_quorum = pallet_tables::TableInsertQuorums::<T>::get(&table);

        let can_submit_for_public_quorum =
            pallet_permissions::Pallet::<T>::ensure_root_or_permissioned(
                origin.clone(),
                &PermissionLevel::IndexingPallet(
                    IndexingPalletPermission::SubmitDataForPublicQuorum,
                ),
            )
            .is_ok()
                && table_insert_quorum.public.is_some();

        let can_submit_for_privileged_quorum =
            pallet_permissions::Pallet::<T>::ensure_root_or_permissioned(
                origin,
                &PermissionLevel::IndexingPallet(
                    IndexingPalletPermission::SubmitDataForPrivilegedQuorum(table.clone()),
                ),
            )
            .is_ok()
                && table_insert_quorum.privileged.is_some();

        ensure!(
            can_submit_for_public_quorum || can_submit_for_privileged_quorum,
            Error::<T, I>::UnauthorizedSubmitter
        );

        validate_submission::<T, I>(&table, &batch_id, &data)?;

        let hash_input = (&data, block_number).encode();
        let data_hash = T::Hashing::hash(&hash_input);

        let public_data_quorum = if can_submit_for_public_quorum {
            submit_data_and_find_quorum::<T, I>(
                who.clone(),
                batch_id.clone(),
                data_hash,
                table.clone(),
                &table_insert_quorum,
                &QuorumScope::Public,
            )?
        } else {
            None
        };

        let privileged_data_quorum = if can_submit_for_privileged_quorum {
            submit_data_and_find_quorum::<T, I>(
                who.clone(),
                batch_id,
                data_hash,
                table.clone(),
                &table_insert_quorum,
                &QuorumScope::Privileged,
            )?
        } else {
            None
        };

        if let Some(data_quorum) = public_data_quorum.or(privileged_data_quorum) {
            finalize_quorum::<T, I>(data_quorum, data, block_number)?;
        }

        Ok(())
    }

    /// Submit data and check if we have a quorum.
    ///
    /// If quorum is reached, the associated [`DataQuorum`] is returned, otherwise returns `None`.
    #[allow(clippy::type_complexity)]
    fn submit_data_and_find_quorum<T, I>(
        who: T::AccountId,
        batch_id: BatchId,
        data_hash: T::Hash,
        table: TableIdentifier,
        table_insert_quorum: &InsertQuorumSize,
        quorum_scope: &QuorumScope,
    ) -> Result<Option<DataQuorum<T::AccountId, T::Hash>>, DispatchError>
    where
        T: Config<I>,
        I: NativeApi,
    {
        let submission_map_with_this =
            SubmissionsV1::<T, I>::iter_prefix((&batch_id, quorum_scope))
                .take(MAX_SUBMITTERS as usize)
                .chain(core::iter::once((who.clone(), data_hash)))
                .collect::<BTreeMap<_, _>>();

        if submission_map_with_this.len() > MAX_SUBMITTERS as usize {
            Err(Error::MaxSubmittersReached::<T, I>)?;
        }
        SubmissionsV1::<T, I>::insert((&batch_id, quorum_scope, &who), data_hash);

        let submission = DataSubmission {
            table: table.clone(),
            batch_id: batch_id.clone(),
            data_hash,
            quorum_scope: *quorum_scope,
        };
        // Emit an event noting who submitted what
        Pallet::<T, I>::deposit_event(Event::DataSubmitted { who, submission });

        let (agreements_unbounded, dissents_unbounded): (BTreeSet<_>, BTreeSet<_>) =
            submission_map_with_this
                .into_iter()
                .partition_map(|(account_id, hash)| {
                    if hash == data_hash {
                        Either::Left(account_id)
                    } else {
                        Either::Right(account_id)
                    }
                });

        match table_insert_quorum.of_scope(quorum_scope) {
            Some(quorum_size) if agreements_unbounded.len() as u8 > *quorum_size => {
                let block_number = <frame_system::Pallet<T>>::block_number();

                const ALREADY_VALIDATED_SUBMITTER_COUNT: &str =
                    "we've already validated that the submitter count is within bounds";
                let agreements = agreements_unbounded
                    .try_into()
                    .expect(ALREADY_VALIDATED_SUBMITTER_COUNT);
                let dissents = dissents_unbounded
                    .try_into()
                    .expect(ALREADY_VALIDATED_SUBMITTER_COUNT);

                // Decide on the quorum
                let data_quorum = DataQuorum {
                    table,
                    batch_id,
                    data_hash,
                    block_number: block_number.into(),
                    agreements,
                    dissents,
                    quorum_scope: *quorum_scope,
                };

                Ok(Some(data_quorum))
            }
            _ => Ok(None),
        }
    }

    /// Performs all steps necessary after reaching quorum, such as...
    /// - recording final data
    /// - committing to data
    /// - emitting `QuorumReached` event
    /// - cleaning up submissions
    fn finalize_quorum<T, I>(
        quorum: DataQuorum<T::AccountId, T::Hash>,
        row_data: RowData,
        block_number: Option<u64>,
    ) -> DispatchResult
    where
        T: Config<I>,
        I: NativeApi,
    {
        SubmissionsV1::<T, I>::clear_prefix(
            (&quorum.batch_id,),
            MAX_SUBMITTERS * QuorumScope::VARIANT_COUNT as u32,
            None,
        );

        FinalData::<T, I>::insert(&quorum.batch_id, &quorum);

        let table_bytes = I::record_batch_to_onchain(sxt_core::native::RowData { row_data })
            .map_err(Error::<T, I>::from)?;

        let oc_table = OnChainTable::try_from(table_bytes)
            .map_err(|_| Error::<T, I>::TableDeserializationError)?;

        let InsertAndCommitmentMetadata {
            insert_with_meta_columns,
            ..
        } = pallet_commitments::Pallet::<T>::process_insert_and_update_commitments::<I>(
            quorum.table.clone(),
            oc_table.clone(),
        )?;

        let on_chain_table_bytes: BoundedVec<u8, ConstU32<DATA_MAX_LEN>> =
            postcard::to_allocvec(&insert_with_meta_columns)
                .map_err(|_| Error::<T, I>::TableSerializationError)?
                .try_into()
                .map_err(|_| Error::<T, I>::TableSerializationError)?;

        if let Some(bn) =
            block_number.or_else(|| oc_table.max_block_number().and_then(|v| v.try_into().ok()))
        {
            BlockNumbers::<T, I>::insert(&quorum.table, bn);
        }

        if oc_table.num_rows() == 0 {
            Pallet::<T, I>::deposit_event(Event::QuorumEmptyBlock {
                table: quorum.table.clone(),
                block_number: block_number.unwrap_or_default(),
                agreements: quorum.agreements.clone(),
                dissents: quorum.dissents.clone(),
            });
        } else {
            Pallet::<T, I>::deposit_event(Event::QuorumReached {
                quorum: quorum.clone(),
                data: on_chain_table_bytes.clone(),
            });
        }

        if quorum.table.is_staking_table() {
            if let Err(e) = pallet_system_tables::Pallet::<T>::process_system_table(
                quorum.table.clone(),
                oc_table,
            ) {
                Pallet::<T, I>::deposit_event(Event::SystemTableError {
                    table: quorum.table.clone(),
                    error: e,
                    data: on_chain_table_bytes,
                });
            } else {
                Pallet::<T, I>::deposit_event(Event::SystemTableUpdate {
                    table: quorum.table.clone(),
                    data: on_chain_table_bytes,
                });
            }
        }

        Ok(())
    }

    /// Run some checks to verify that table, batch_id, and data are reasonable, non-empty values\
    /// If the transaction is considered invalid, a relevant error will be returned
    pub fn validate_submission<T, I>(
        table: &TableIdentifier,
        batch_id: &BatchId,
        data: &RowData,
    ) -> DispatchResult
    where
        T: Config<I>,
        I: NativeApi,
    {
        // Check if this batch has already been decided on
        if FinalData::<T, I>::contains_key(batch_id) {
            Err(Error::<T, I>::LateBatch)?
        }

        ensure!(
            !(table.namespace.is_empty() || table.name.is_empty()),
            Error::<T, I>::InvalidTable
        );
        ensure!(!data.is_empty(), Error::<T, I>::NoData);
        ensure!(!batch_id.is_empty(), Error::<T, I>::InvalidBatch);
        // Make sure the schema exists for this table
        ensure!(
            pallet_tables::Schemas::<T>::contains_key(&table.namespace, &table.name),
            Error::<T, I>::InvalidTable
        );
        Ok(())
    }
}
