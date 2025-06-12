//! Migration from pallet-indexing storage v1 to v2
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

/// Populates the `BatchQueue` from existing `Submissions`.
///
/// As we cannot know the order that the batches were submitted from current storage information,
/// the batches will be represented in the queue in their `SubmissionsV1` iteration order
/// (lexicographic).
///
/// This will not do any pruning, even if current submissions exceed the `BATCH_QUEUE_CAPACITY`.
/// So, it is recommended to start using the batch queue with a capacity higher than any live
/// network's current submission count, to avoid a bunch of batches getting pruned on the very next
/// submission.
pub struct LazyMigrationV2<T: Config<I>, W: crate::weights::WeightInfo, I: 'static = ()>(
    PhantomData<(T, W, I)>,
);

impl<T: Config<I>, W: crate::weights::WeightInfo, I: 'static> SteppedMigration
    for LazyMigrationV2<T, W, I>
{
    type Cursor = (BatchId, QuorumScope, T::AccountId);
    type Identifier = MigrationId<26>;

    fn id() -> Self::Identifier {
        MigrationId {
            pallet_id: *PALLET_MIGRATIONS_ID,
            version_from: 1,
            version_to: 2,
        }
    }

    fn step(
        mut cursor: Option<Self::Cursor>,
        meter: &mut WeightMeter,
    ) -> Result<Option<Self::Cursor>, SteppedMigrationError> {
        let required = W::migration_v1_v2_step();

        if meter.remaining().any_lt(required) {
            return Err(SteppedMigrationError::InsufficientWeight { required });
        }

        // We loop here to do as much progress as possible per step.
        loop {
            if meter.try_consume(required).is_err() {
                break;
            }

            let mut iter = if let Some(key) = &cursor {
                // If a cursor is provided, start iterating from the stored value
                SubmissionsV1::<T, I>::iter_keys_from(SubmissionsV1::<T, I>::hashed_key_for(key))
            } else {
                // If no cursor is provided, start iterating from the beginning.
                SubmissionsV1::<T, I>::iter_keys()
            };

            // If there's a next item in the iterator, perform the migration.
            if let Some((batch_id, scope, account)) = iter.next() {
                let batch_index = crate::BatchQueue::<T, I>::count();
                crate::BatchQueue::<T, I>::insert(batch_index as u64, batch_id.clone());

                // update cursor always
                cursor = Some((batch_id.clone(), scope, account));

                // update cursor to the last key w/ this batch id, skipping them in the migration
                for key in iter {
                    if key.0 == batch_id {
                        cursor = Some(key);
                    } else {
                        break;
                    }
                }
            } else {
                cursor = None; // Signal that the migration is complete (no more items to process).
                break;
            }
        }
        Ok(cursor)
    }
}
