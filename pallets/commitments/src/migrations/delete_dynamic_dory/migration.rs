use alloc::vec::Vec;
use core::marker::PhantomData;

use polkadot_sdk::frame_support::ensure;
use polkadot_sdk::frame_support::migrations::{
    MigrationId,
    SteppedMigration,
    SteppedMigrationError,
};
use polkadot_sdk::frame_support::weights::WeightMeter;
use proof_of_sql_commitment_map::CommitmentScheme;
use sxt_core::tables::TableIdentifier;

use crate::migrations::PALLET_MIGRATIONS_ID;
use crate::{CommitmentStorageMap, Config};

/// Due to an increase in the cost of insertions to dynamic dory tables, we disable dynamic dory
/// for existing tables by simply deleting those commitments.
pub struct DeleteDynamicDoryCommitmentsLazyMigration<T: Config, W: super::weights::WeightInfo>(
    PhantomData<(T, W)>,
);

/// An extra limit to avoid significant single block lag from any weight misrepresentations.
pub const MAX_DELETIONS_PER_BLOCK: usize = 5;

impl<T: Config, W: super::weights::WeightInfo> SteppedMigration
    for DeleteDynamicDoryCommitmentsLazyMigration<T, W>
{
    type Cursor = (TableIdentifier, CommitmentScheme);
    type Identifier = MigrationId<29>;

    fn id() -> Self::Identifier {
        MigrationId {
            pallet_id: *PALLET_MIGRATIONS_ID,
            version_from: 0,
            version_to: 1,
        }
    }

    fn step(
        cursor: Option<Self::Cursor>,
        meter: &mut WeightMeter,
    ) -> Result<Option<Self::Cursor>, SteppedMigrationError> {
        let required = W::step();
        // If there is not enough weight for a single step, return an error. This case can be
        // problematic if it is the first migration that ran in this block. But there is nothing
        // that we can do about it here.
        ensure!(
            meter.remaining().all_gte(required),
            SteppedMigrationError::InsufficientWeight { required }
        );

        let max_sub_steps = meter
            .remaining()
            .checked_div_per_component(&required)
            // This should not happen so long as `required` is non-zero
            .ok_or(SteppedMigrationError::Failed)?
            .try_into()
            // can only happen if the u64 exceeds usize::MAX, so clamp.
            .unwrap_or(usize::MAX)
            .min(MAX_DELETIONS_PER_BLOCK);

        let table_entries: Vec<_> = cursor
            .map_or_else(
                CommitmentStorageMap::<T>::iter_keys,
                |(last_table_identifier, last_commitment_scheme)| {
                    CommitmentStorageMap::<T>::iter_keys_from(
                        CommitmentStorageMap::<T>::hashed_key_for(
                            last_table_identifier,
                            last_commitment_scheme,
                        ),
                    )
                },
            )
            .take(max_sub_steps)
            .collect();

        let reached_end = table_entries.len() < max_sub_steps;

        // If we reached the end of the iterator (found less than max_sub_steps entries),
        // we technically did 1 extra read operation to see that it was indeed the end.
        let num_read = table_entries.len().saturating_add(reached_end.into());

        // Consume the weight.
        meter
            .try_consume(
                required.saturating_mul(
                    num_read
                        .try_into()
                        // this should not happen, usize is smaller than u64 in our runtime
                        .map_err(|_| SteppedMigrationError::Failed)?,
                ),
            )
            // this should not happen, num_read should be <= weight / required
            .map_err(|_| SteppedMigrationError::Failed)?;

        // Perform the removals.
        table_entries
            .iter()
            .filter(|(_, commitment_scheme)| commitment_scheme == &CommitmentScheme::DynamicDory)
            .for_each(|(table_identifier, _)| {
                CommitmentStorageMap::<T>::remove(table_identifier, CommitmentScheme::DynamicDory)
            });

        if reached_end {
            Ok(None)
        } else {
            Ok(table_entries.last().cloned())
        }
    }
}
