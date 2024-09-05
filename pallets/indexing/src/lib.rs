//! The Indexing Pallet
//! The indexing pallet implements the functionality needed to allow indexers to submit data
//! via the `submit_data` extrinsic. Once data is submitted, it contains logic for checking if
//! we have enough submissions to reach a quorum, and if we do, it will finalize the data and
//! emit an event stating that the batch id has been decided on. The event also contains the
//! final data for the decision.

// We make sure this pallet uses `no_std` for compiling to Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;
pub mod weights;
pub use weights::*;

pub use sxt_core::indexing::*;

#[allow(clippy::manual_inspect)]
#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use crate::Event::*;
    use frame_support::pallet_prelude::*;
    
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::Hash;

    use sxt_core::permissions::{IndexingPalletPermission, PermissionLevel};
    use sxt_core::tables::TableIdentifier;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_permissions::Config {
        /// Binding for the runtime event, typically provided by an implementation
        /// in runtime/lib.rs
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// The weight info to be used with the extrinsics provided by the pallet
        type WeightInfo: WeightInfo;
    }

    /// Double Map of Submissions using the batch-id as the first key and the submitter's
    /// public key as the second key to hold the hash of the submitted data.
    /// Each submission for a given batch id will have an entry here
    #[pallet::storage]
    #[pallet::getter(fn submissions)]
    pub type Submissions<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        BatchId,
        Blake2_128Concat,
        <T as frame_system::Config>::Hash,
        SubmitterList<T::AccountId>,
        ValueQuery, // Allows us to receive a default instead of None
    >;

    #[pallet::storage]
    #[pallet::getter(fn final_data)]
    pub type FinalData<T: Config> =
        StorageMap<_, Blake2_128Concat, BatchId, DataQuorum<T::AccountId, T::Hash>>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
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
            /// The finalized raw data in RecordBatch IPC format
            data: RowData,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
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
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// This extrinsic provides a transaction that indexers will use to submit
        /// data they've indexed.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::submit_data())]
        pub fn submit_data(
            origin: OriginFor<T>,
            table: TableIdentifier,
            batch_id: BatchId,
            data: RowData,
        ) -> DispatchResult {
            // Check that the extrinsic was signed and get the signer.
            // This function will return an error if the extrinsic is not signed.
            // https://docs.substrate.io/main-docs/build/origins/
            let who = ensure_signed(origin.clone())?;
            pallet_permissions::Pallet::<T>::ensure_root_or_permissioned(
                origin,
                &PermissionLevel::IndexingPallet(IndexingPalletPermission::SubmitData),
            )
            .or(Err(Error::<T>::UnauthorizedSubmitter))?;

            // Run the basic checks on the submissions
            validate_submission::<T>(&table, &batch_id, &data)?;

            // Check if this batch has already been decided on
            if FinalData::<T>::contains_key(&batch_id) {
                Err(Error::<T>::LateBatch)?
            }

            let data_hash = T::Hashing::hash(&data);

            // We don't need to save the full data. We just need a count associated with each submission
            let mut match_submissions = Submissions::<T>::get(&batch_id, data_hash);

            // Check if this user has already submitted this data
            if match_submissions.contains(&who) {
                Err(Error::<T>::AlreadySubmitted)?
            }

            let _ = match_submissions.try_push(who.clone());
            Submissions::<T>::insert(&batch_id, data_hash, match_submissions.clone());

            let submission = DataSubmission {
                table: table.clone(),
                batch_id: batch_id.clone(),
                data_hash,
            };

            // Emit an event noting who submitted what
            Self::deposit_event(DataSubmitted { who, submission });

            // 3 is a temporary number here until we get Indexer staking/registration integrated
            if match_submissions.len() > 3 {
                check_quorum_and_finalize::<T>(batch_id, data_hash, data, table, match_submissions);
            }

            // Return a successful Result
            Ok(())
        }
    }

    /// Check if we have a quorum. Finalize the data and emit an event if we do
    pub fn check_quorum_and_finalize<T: Config>(
        batch_id: BatchId,
        data_hash: T::Hash,
        data: RowData,
        table: TableIdentifier,
        match_submissions: SubmitterList<T::AccountId>,
    ) {
        // Iterate over the submitters who submitted differing data and collect
        // their account ids
        let dissenters = Submissions::<T>::iter_prefix(&batch_id)
            .filter(|(hash, _)| hash != &data_hash)
            .flat_map(|(_, submitters)| submitters)
            .take(MAX_SUBMITTERS as usize)
            .collect::<alloc::vec::Vec<_>>()
            .try_into()
            .expect("source Vec is constructed to not exceed maximum submitter list size");

        // Cleanup other entries from the state and log that we chose this data for this batch_id
        // Use an iterator to find all keys with the given prefix and remove them
        // We are also using this opportunity to identify the dissenting votes
        Submissions::<T>::iter_prefix(&batch_id).for_each(|(second_key, _)| {
            // Remove the entry associated with the first key and each second key
            Submissions::<T>::remove(&batch_id, second_key);
        });

        // Decide on the quorum
        let quorum = DataQuorum {
            table,
            batch_id: batch_id.clone(),
            data_hash,
            block_number: <frame_system::Pallet<T>>::block_number().into(),
            agreements: match_submissions,
            dissents: dissenters,
        };

        // Save the final data that we decided on
        FinalData::<T>::insert(&batch_id, quorum.clone());

        // Emit an event.
        Pallet::<T>::deposit_event(QuorumReached { quorum, data });
    }

    /// Run some checks to verify that table, batch_id, and data are reasonable, non-empty values\
    /// If the transaction is considered invalid, a relevant error will be returned
    pub fn validate_submission<T: Config>(
        table: &TableIdentifier,
        batch_id: &BatchId,
        data: &RowData,
    ) -> DispatchResult {
        ensure!(
            !(table.namespace.is_empty() || table.name.is_empty()),
            Error::<T>::InvalidTable
        );
        ensure!(!data.is_empty(), Error::<T>::NoData);
        ensure!(!batch_id.is_empty(), Error::<T>::InvalidBatch);
        Ok(())
    }
}
