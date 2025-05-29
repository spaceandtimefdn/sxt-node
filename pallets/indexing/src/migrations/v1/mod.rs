//! Migration from pallet-indexing storage v0 to v1
//!
//! This migration handles changes to the `Submitters`, which went from being a mapping of data
//! hashes to submitters to the reverse.

use frame_support::migrations::{MigrationId, SteppedMigration, SteppedMigrationError};
use frame_support::pallet_prelude::PhantomData;
use frame_support::weights::WeightMeter;
use sxt_core::indexing::BatchId;
use sxt_core::tables::QuorumScope;

use super::PALLET_MIGRATIONS_ID;
use crate::pallet::{Config, SubmissionsV1};

mod tests;

/// Module containing the OLD (v0) storage Submissions.
#[allow(missing_docs)]
pub mod v0 {
    use frame_support::pallet_prelude::ValueQuery;
    use frame_support::{storage_alias, Blake2_128Concat};
    use sxt_core::indexing::{BatchId, SubmittersByScope};

    use super::Config;
    use crate::pallet::Pallet;

    /// The Submissions that is being migrated from.
    #[storage_alias]
    pub type Submissions<T: Config<I>, I: 'static> = StorageDoubleMap<
        Pallet<T, I>,
        Blake2_128Concat,
        BatchId,
        Blake2_128Concat,
        <T as frame_system::Config>::Hash,
        SubmittersByScope<<T as frame_system::Config>::AccountId>,
        ValueQuery,
    >;
}

/// Migrates [`crate::Submissions`]'s map structure
///
/// From:
/// `batch_id -> data_hash -> quorum_scope -> submitter_list`
/// (Though that last mapping is just in the form of a struct)
///
/// To:
/// `batch_id -> quorum_scope -> submitter -> data_hash`
pub struct LazyMigrationV1<T: Config<I>, W: crate::weights::WeightInfo, I: 'static = ()>(
    PhantomData<(T, W, I)>,
);

impl<T: Config<I>, W: crate::weights::WeightInfo, I: 'static> SteppedMigration
    for LazyMigrationV1<T, W, I>
{
    type Cursor = (BatchId, <T as frame_system::Config>::Hash);
    type Identifier = MigrationId<26>;

    fn id() -> Self::Identifier {
        MigrationId {
            pallet_id: *PALLET_MIGRATIONS_ID,
            version_from: 0,
            version_to: 1,
        }
    }

    fn step(
        mut cursor: Option<Self::Cursor>,
        meter: &mut WeightMeter,
    ) -> Result<Option<Self::Cursor>, SteppedMigrationError> {
        let required = W::migration_v0_v1_step();

        if meter.remaining().any_lt(required) {
            return Err(SteppedMigrationError::InsufficientWeight { required });
        }

        // We loop here to do as much progress as possible per step.
        loop {
            if meter.try_consume(required).is_err() {
                break;
            }

            let mut iter = if let Some((last_batch_id, last_hash)) = cursor {
                // If a cursor is provided, start iterating from the stored value
                v0::Submissions::<T, I>::iter_from(v0::Submissions::<T, I>::hashed_key_for(
                    last_batch_id,
                    last_hash,
                ))
            } else {
                // If no cursor is provided, start iterating from the beginning.
                v0::Submissions::<T, I>::iter()
            };

            // If there's a next item in the iterator, perform the migration.
            if let Some((batch_id, data_hash, submitters_by_scope)) = iter.next() {
                submitters_by_scope
                    .iter_scope(&QuorumScope::Public)
                    .for_each(|submitter| {
                        SubmissionsV1::<T, I>::insert(
                            (batch_id.clone(), QuorumScope::Public, submitter),
                            data_hash,
                        );
                    });
                submitters_by_scope
                    .iter_scope(&QuorumScope::Privileged)
                    .for_each(|submitter| {
                        SubmissionsV1::<T, I>::insert(
                            (batch_id.clone(), QuorumScope::Privileged, submitter),
                            data_hash,
                        );
                    });

                v0::Submissions::<T, I>::remove(&batch_id, data_hash);

                cursor = Some((batch_id, data_hash)) // Return the processed key as the new cursor.
            } else {
                cursor = None; // Signal that the migration is complete (no more items to process).
                break;
            }
        }
        Ok(cursor)
    }
}
