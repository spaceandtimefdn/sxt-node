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

#[allow(clippy::manual_inspect)]
#[frame_support::pallet]
pub mod pallet {
    use alloc::collections::{BTreeMap, BTreeSet};
    use alloc::string::String;
    use alloc::vec::Vec;

    use codec::{Decode, EncodeLike};
    use commitment_sql::InsertAndCommitmentMetadata;
    use frame_support::dispatch::RawOrigin;
    use frame_support::pallet_prelude::*;
    use frame_support::{Blake2_128, Blake2_128Concat};
    use frame_system::pallet_prelude::*;
    use hex::FromHex;
    use itertools::Itertools;
    use native_api::NativeApi;
    use on_chain_table::OnChainTable;
    use sp_core::crypto::Ss58Codec;
    use sp_core::{H256, U256};
    use sp_runtime::traits::{Bounded, Hash, StaticLookup, UniqueSaturatedInto};
    use sp_runtime::{BoundedVec, Either, SaturatedConversion};
    use sxt_core::heavy::Heavy;
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

        /// The maximum batches finding quorum at any given time.
        #[pallet::constant]
        type MaxBatchesFindingQuorum: Get<u32>;

        /// The maximum batches pruned per transaction from submissions storage when it exceeds
        /// `MaxBatchesPruned`.
        #[pallet::constant]
        type MaxBatchesPruned: Get<u32>;
    }

    /// Storage map of `BatchId` and data hash to submitters that have agreed to the batch/hash.
    #[pallet::storage]
    pub type Submissions<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        BatchId,
        Blake2_128Concat,
        <T as frame_system::Config>::Hash,
        SubmittersByScope<T::AccountId>,
        ValueQuery, // Allows us to receive a default instead of None
    >;

    /// Storage map of `BatchId`s to `DataQuorum`s for batches that have reached quorum.
    #[pallet::storage]
    #[pallet::getter(fn submissions_v1)]
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
    #[pallet::getter(fn batch_queue_get)]
    pub type BatchQueue<T: Config<I>, I: 'static = ()> =
        CountedStorageMap<_, Blake2_128Concat, u32, BatchId>;

    #[pallet::storage]
    #[pallet::getter(fn batch_queue_bottom)]
    pub type BatchQueueBottom<T: Config<I>, I: 'static = ()> = StorageValue<_, u32, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn final_data)]
    pub type FinalData<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128Concat, BatchId, DataQuorum<T::AccountId, T::Hash>>;

    /// Storate map of `TableIdentifier`s to block numbers.
    ///
    /// Updated during inserts if the table has a `BLOCK_NUMBER` column, or if
    /// `submit_blockchain_data` is used for the insert.
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
            /// The submission that was sent. Only contains the hash of the original data.
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
        /// Emitted when an insert for a system table has reached quorum, potentially causing
        /// further on-chain actions per row.
        SystemTableUpdate {
            /// The table that was updated
            table: TableIdentifier,
            /// The postcard serialized OnChainTable bytes for the system table insert
            data: BoundedVec<u8, ConstU32<DATA_MAX_LEN>>,
        },
        /// Emitted when the additional processing of system table inserts encounters an error.
        SystemTableError {
            /// The table that had an error
            table: TableIdentifier,
            /// The error received while processing the insert
            error: DispatchError,
            /// The postcard serialized OnChainTable bytes for the system table insert
            data: BoundedVec<u8, ConstU32<DATA_MAX_LEN>>,
        },

        /// Quorum has been reached for an empty blockchain data insert.
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

        BatchQueuePruned {
            num_pruned: u32,
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
        /// This user has already submitted data for this batch id
        AlreadySubmitted,
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
        /// Error deserializing the table as an OnChainTable
        TableDeserializationError,
        /// Error deserializing the table as an OnChainTable
        TableSerializationError,
        /// Submitter Injection Failed
        SubmitterInjectionFailed,
        /// Maximum submissions already reached for this batch id
        MaxSubmittersReached,
    }

    #[pallet::call]
    impl<T: Config<I>, I: 'static> Pallet<T, I>
    where
        T: pallet_tables::Config,
        I: NativeApi,
    {
        /// Submit an IPC-formatted record batch for a given table.
        ///
        /// Submissions go through a quorum-finding process before actually resulting in a table
        /// insert. The quorum size required is defined by the table in the tables-pallet. For
        /// public tables, this quorum size is 0 by default, in which case submissions reach quorum
        /// immediately.
        ///
        /// If the table is a system table, additional chain state transitions may be performed
        /// once quorum is reached.
        ///
        /// # Events
        /// Emits..
        /// - `Event::DataSubmitted`
        /// - `Event::QuorumReached`
        /// - `Event::QuorumEmptyBlock`
        /// - `Event::SystemTableUpdate`
        /// - `Event::SystemTableError`
        ///
        /// # Permissions
        /// Requires either..
        /// - `PalletIndexingPermissions::SubmitDataForPublicQuorum` for tables with a public
        /// quorum size.
        /// - `PalletIndexingPermissions::SubmitDataForPrivilegedQuorum(table)` for tables with a
        /// privileged quorum size.
        /// - the table to be public-permissionless
        #[pallet::call_index(0)]
        #[pallet::weight(submit_data_weight::<T, I>())]
        pub fn submit_data(
            origin: OriginFor<T>,
            table: TableIdentifier,
            batch_id: BatchId,
            data: RowData,
        ) -> DispatchResult {
            submit_data_inner::<T, I>(origin, table, batch_id, data, None)
        }

        /// Submit an IPC-formatted record batch for a given table with block number metadata.
        ///
        /// The block number is stored to assist coordination among decentralized submitters.
        ///
        /// Submissions go through a quorum-finding process before actually resulting in a table
        /// insert. The quorum size required is defined by the table in the tables-pallet. For
        /// public tables, this quorum size is 0 by default, in which case submissions reach quorum
        /// immediately.
        ///
        /// If the table is a system table, additional chain state transitions may be performed
        /// once quorum is reached.
        ///
        /// # Events
        /// Emits..
        /// - `Event::DataSubmitted`
        /// - `Event::QuorumReached`
        /// - `Event::QuorumEmptyBlock`
        /// - `Event::SystemTableUpdate`
        /// - `Event::SystemTableError`
        ///
        /// # Permissions
        /// Requires either..
        /// - `PalletIndexingPermissions::SubmitDataForPublicQuorum` for tables with a public
        /// quorum size.
        /// - `PalletIndexingPermissions::SubmitDataForPrivilegedQuorum(table)` for tables with a
        /// privileged quorum size.
        /// - the table to be public-permissionless
        #[pallet::call_index(1)]
        #[pallet::weight(submit_data_weight::<T, I>())]
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

    fn submit_data_weight<T, I>() -> Weight
    where
        T: Config<I>,
        I: NativeApi,
    {
        let submit_no_quorum = <SubstrateWeight<T> as WeightInfo>::submit_data_quorum_not_reached();
        let submit_w_quorum = <SubstrateWeight<T> as WeightInfo>::submit_data_quorum_reached();

        // Assume in 4 submissions, one will have a quorum event
        let submit_avg_time = ((3 * submit_no_quorum.ref_time()) + submit_w_quorum.ref_time()) / 4;
        let submit_avg_proof =
            ((3 * submit_no_quorum.proof_size()) + submit_w_quorum.proof_size()) / 4;
        Weight::from_parts(submit_avg_time, submit_avg_proof)
    }

    fn submit_data_inner<T, I>(
        origin: OriginFor<T>,
        table: TableIdentifier,
        outer_batch_id: BatchId,
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

        let is_permissionless_insert =
            pallet_tables::Identifiers::<T>::get(sxt_core::tables::TableType::PublicPermissionless)
                .contains(&table);

        ensure!(
            can_submit_for_public_quorum
                || can_submit_for_privileged_quorum
                || is_permissionless_insert,
            Error::<T, I>::UnauthorizedSubmitter
        );

        ensure!(
            !is_legacy_duplicate::<T, I>(&outer_batch_id, &table),
            Error::<T, I>::LateBatch
        );

        ensure!(!outer_batch_id.is_empty(), Error::<T, I>::InvalidBatch);

        let batch_id = build_inner_batch_id::<T, I>(&outer_batch_id, &table);

        validate_submission::<T, I>(&table, &batch_id, &data)?;

        let hash_input = (&data, block_number).encode();
        let data_hash = T::Hashing::hash(&hash_input);

        let opt_data_quorum = if can_submit_for_privileged_quorum {
            submit_data_and_find_quorum::<T, I>(
                who.clone(),
                batch_id,
                data_hash,
                table.clone(),
                &table_insert_quorum,
                &QuorumScope::Privileged,
            )?
        } else if can_submit_for_public_quorum || is_permissionless_insert {
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

        if let Some(data_quorum) = opt_data_quorum {
            finalize_quorum::<T, I>(data_quorum, data, block_number, who)?;
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
        // There is no `StorageNMap::contains_key_prefix`
        if SubmissionsV1::<T, I>::iter_prefix((&batch_id,))
            .next()
            .is_none()
        {
            let batch_index = Pallet::<T, I>::batch_queue_bottom() + BatchQueue::<T, I>::count();
            BatchQueue::<T, I>::insert(&batch_index, &batch_id);
        }

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

                // Technically we don't need to check this, we know at this point that both lists
                // sizes will sum up to the size of submission_map_with_this, which we already
                // checked is below the number of max submitters. We still avoid the panic out of
                // an abundance of caution.
                let (agreements, dissents) = agreements_unbounded
                    .try_into()
                    .and_then(|agreements| Ok((agreements, dissents_unbounded.try_into()?)))
                    .map_err(|_| Error::MaxSubmittersReached::<T, I>)?;

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
        submitter: T::AccountId,
    ) -> DispatchResult
    where
        T: Config<I>,
        I: NativeApi,
    {
        // Clean up submissions for this batch
        let _ = remove_batch_id_from_submissions_v1::<T, I>(&quorum.batch_id);

        // Record final decision
        FinalData::<T, I>::insert(&quorum.batch_id, &quorum);

        // Deserialize into Arrow-compatible OnChainTable
        let table_bytes = I::record_batch_to_onchain(sxt_core::native::RowData { row_data })
            .map_err(Error::<T, I>::from)?;

        let oc_table = OnChainTable::try_from(table_bytes)
            .map_err(|_| Error::<T, I>::TableDeserializationError)?;

        // Check if the table is permissionless and inject the submitter column if needed
        let is_permissionless =
            pallet_tables::Identifiers::<T>::get(sxt_core::tables::TableType::PublicPermissionless)
                .contains(&quorum.table);

        let oc_table = if is_permissionless {
            sxt_core::tables::inject_submitter_data(oc_table, submitter.encode())
                .map_err(|_| Error::<T, I>::SubmitterInjectionFailed)?
        } else {
            oc_table
        };

        // Commit to the data and retrieve metadata
        let InsertAndCommitmentMetadata {
            insert_with_meta_columns,
            ..
        } = pallet_commitments::Pallet::<T>::process_insert_and_update_commitments::<I>(
            quorum.table.clone(),
            oc_table.clone(),
        )?;

        // Serialize the final data
        let on_chain_table_bytes: BoundedVec<u8, ConstU32<DATA_MAX_LEN>> =
            postcard::to_allocvec(&insert_with_meta_columns)
                .map_err(|_| Error::<T, I>::TableSerializationError)?
                .try_into()
                .map_err(|_| Error::<T, I>::TableSerializationError)?;

        // Update latest indexed block number if applicable
        if let Some(bn) =
            block_number.or_else(|| oc_table.max_block_number().and_then(|v| v.try_into().ok()))
        {
            BlockNumbers::<T, I>::insert(&quorum.table, bn);
        }

        // Emit appropriate event
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

        // If system table, propagate insert or error
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

    pub(crate) fn build_inner_batch_id<T, I>(
        outer_batch_id: &BatchId,
        table: &TableIdentifier,
    ) -> BatchId
    where
        T: Config<I>,
        I: NativeApi,
    {
        BatchId::truncate_from(
            T::Hashing::hash(&(&table, outer_batch_id).encode())
                .as_ref()
                .to_vec(),
        )
    }

    pub(crate) fn is_legacy_duplicate<T, I>(
        outer_batch_id: &BatchId,
        table_id: &TableIdentifier,
    ) -> bool
    where
        T: Config<I>,
        I: NativeApi,
    {
        if let Some(old_final_data) = FinalData::<T, I>::get(outer_batch_id) {
            &old_final_data.table == table_id
        } else {
            false
        }
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
        // Make sure the schema exists for this table
        ensure!(
            pallet_tables::Schemas::<T>::contains_key(&table.namespace, &table.name),
            Error::<T, I>::InvalidTable
        );
        Ok(())
    }

    /// Returns the count of `BatchQueue` and the weight of the get.
    fn batch_queue_count_heavy<T, I>() -> Heavy<u32>
    where
        T: Config<I>,
        I: NativeApi,
    {
        let out = BatchQueue::<T, I>::count();
        let weight = T::DbWeight::get().reads(1);
        Heavy { out, weight }
    }

    /// Returns the value of `BatchQueueBottom` and the weight of the get.
    fn batch_queue_bottom_heavy<T, I>() -> Heavy<u32>
    where
        T: Config<I>,
        I: NativeApi,
    {
        let out = Pallet::<T, I>::batch_queue_bottom();
        let weight = T::DbWeight::get().reads(1);
        Heavy { out, weight }
    }

    /// Sets the value of `BatchQueueBottom` and returns the weight of the set.
    fn batch_queue_bottom_set_heavy<T, I>(bottom: u32) -> Heavy<()>
    where
        T: Config<I>,
        I: NativeApi,
    {
        BatchQueueBottom::<T, I>::set(bottom);
        T::DbWeight::get().writes(1).into()
    }

    /// Removes and returns the `BatchId` at the given index in the `BatchQueue`, along with the
    /// weight of the take.
    fn batch_queue_take_heavy<T, I>(batch_index: u32) -> Heavy<Option<BatchId>>
    where
        T: Config<I>,
        I: NativeApi,
    {
        let out = BatchQueue::<T, I>::take(batch_index);

        let weight = if out.is_some() {
            T::DbWeight::get().reads_writes(1, 1)
        } else {
            T::DbWeight::get().reads(1)
        };

        Heavy { out, weight }
    }

    /// Removes up to `prune_limit` entries from the v0 `Submissions` storage.
    ///
    /// Returns what remains of the prune limit, i.e., `prune_limit - num_pruned`.
    fn prune_submissions_v0<T, I>(prune_limit: u32) -> Heavy<u32>
    where
        T: Config<I>,
        I: NativeApi,
    {
        // In testing, `StorageDoubleMap::clear` didn't obey the limits, removing all entries
        // instead. So, this does a manual iter-keys-take-n-remove instead.

        // Technically, since this is a double map, this clears n (batch_id, data_hash) pairs, not
        // n batch_ids. These won't be 1-to-1 in the case that there was a controversial batch_id.
        // However, any partially-removed batch_id will be cleaned up in future calls.
        let keys_to_remove = Submissions::<T, I>::iter_keys()
            .take(prune_limit.try_into().unwrap_or_default())
            .collect::<Vec<_>>();
        let removal_count = keys_to_remove.len().try_into().unwrap_or_default();

        keys_to_remove
            .into_iter()
            .for_each(|(batch_id, data_hash)| {
                Submissions::<T, I>::remove(batch_id, data_hash);
            });

        let remaining_prunes = prune_limit.saturating_sub(removal_count);
        let weight = T::DbWeight::get().reads_writes(removal_count.into(), removal_count.into());

        Heavy {
            out: remaining_prunes,
            weight,
        }
    }

    /// Removes the given `batch_id` from the `SubmissionsV1` storage and returns the weight of the
    /// clear_prefix.
    fn remove_batch_id_from_submissions_v1<T, I>(batch_id: impl EncodeLike<BatchId>) -> Heavy<()>
    where
        T: Config<I>,
        I: NativeApi,
    {
        let removal_limit = MAX_SUBMITTERS
            .saturating_mul(QuorumScope::VARIANT_COUNT.try_into().unwrap_or_default());

        let removal_results = SubmissionsV1::<T, I>::clear_prefix((batch_id,), removal_limit, None);

        T::DbWeight::get()
            .reads_writes(removal_results.loops.into(), removal_results.unique.into())
            .into()
    }

    /// Removes up to `prune_limit` batches from the `SubmissionsV1` storage.
    ///
    /// Returns what remains of the prune limit, i.e., `prune_limit - num_pruned`.
    fn prune_batch_queue<T, I>(prune_limit: u32) -> Heavy<u32>
    where
        T: Config<I>,
        I: NativeApi,
    {
        batch_queue_count_heavy::<T, I>().and_then(|batch_queue_size| {
            if batch_queue_size <= T::MaxBatchesFindingQuorum::get() {
                // nothing to prune
                return prune_limit.into();
            }

            batch_queue_bottom_heavy::<T, I>().and_then(|batch_queue_bottom| {
                let num_batches_to_prune =
                    batch_queue_size.saturating_sub(T::MaxBatchesFindingQuorum::get());
                let clamped_num_batches_to_prune = num_batches_to_prune.min(prune_limit);

                let new_batch_queue_bottom = batch_queue_bottom + clamped_num_batches_to_prune;

                (batch_queue_bottom..new_batch_queue_bottom)
                    .map(|batch_index| {
                        batch_queue_take_heavy::<T, I>(batch_index).and_then(|batch_id| {
                            if let Some(batch_id) = batch_id {
                                remove_batch_id_from_submissions_v1::<T, I>(batch_id)
                            } else {
                                ().into()
                            }
                        })
                    })
                    .sum::<Heavy<()>>()
                    .and_then(|_| batch_queue_bottom_set_heavy::<T, I>(new_batch_queue_bottom))
                    .map(|_| prune_limit.saturating_sub(clamped_num_batches_to_prune))
            })
        })
    }

    #[pallet::hooks]
    impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I>
    where
        I: NativeApi,
    {
        fn on_initialize(_: BlockNumberFor<T>) -> Weight {
            let max_batches_pruned = T::MaxBatchesPruned::get();

            let Heavy {
                out: remaining_prunes,
                weight,
            } = prune_submissions_v0::<T, I>(max_batches_pruned)
                .and_then(prune_batch_queue::<T, I>);

            let num_pruned = max_batches_pruned.saturating_sub(remaining_prunes);

            if num_pruned > 0 {
                Pallet::<T, I>::deposit_event(Event::<T, I>::BatchQueuePruned { num_pruned });
            }

            weight
        }
    }
}
