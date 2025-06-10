#![cfg(all(test, not(feature = "runtime-benchmarks")))]

use codec::Decode;
use frame_support::traits::OnRuntimeUpgrade;
use frame_system::ensure_signed;
use native_api::Api;
use pallet_migrations::WeightInfo as _;
use sp_core::Hasher;
use sxt_core::indexing::{BatchId, SubmittersByScope};
use sxt_core::tables::QuorumScope;

use crate::migrations::v1;
use crate::mock::{
    new_test_ext,
    run_to_block,
    AllPalletsWithSystem,
    MigratorServiceWeight,
    RuntimeOrigin,
    System,
    Test,
};
use crate::weights::WeightInfo as _;

fn submissions_from_seed(
    seed: u64,
) -> (
    BatchId,
    <Test as frame_system::Config>::Hash,
    SubmittersByScope<<Test as frame_system::Config>::AccountId>,
) {
    let batch_id = BatchId::try_from(seed.to_le_bytes().to_vec()).unwrap();
    let data_hash = <<Test as frame_system::Config>::Hashing as Hasher>::hash(&seed.to_le_bytes());

    let num_privileged_submissions = seed % 32;

    // this math is chosen to
    // - have some variety with num_privileged_submissions
    // - always have at least 1 public submission (simplifies test logic)
    let num_public_submissions = ((seed * 2) % 31) + 1;

    let privileged_submitters_by_scope = (0..num_privileged_submissions)
        .map(|submitter_seed_index| {
            let submitter_index = seed * 32 + submitter_seed_index;
            ensure_signed(RuntimeOrigin::signed(submitter_index)).unwrap()
        })
        .fold(SubmittersByScope::default(), |acc, submitter| {
            acc.with_submitter(submitter, &QuorumScope::Privileged)
                .unwrap()
        });
    let submitters_by_scope = (0..num_public_submissions)
        .map(|submitter_seed_index| {
            let submitter_index = seed * 32 + submitter_seed_index;
            ensure_signed(RuntimeOrigin::signed(submitter_index)).unwrap()
        })
        .fold(privileged_submitters_by_scope, |acc, submitter| {
            acc.with_submitter(submitter, &QuorumScope::Public).unwrap()
        });

    (batch_id, data_hash, submitters_by_scope)
}

#[test]
fn lazy_migration_works() {
    new_test_ext().execute_with(|| {
        (0..64u64)
            .map(submissions_from_seed)
            .for_each(|(batch_id, data_hash, submitters)| {
                submitters
                    .iter_scope(&QuorumScope::Public)
                    .for_each(|submitter| {
                        crate::SubmissionsV1::<Test, Api>::insert(
                            (&batch_id, QuorumScope::Public, &submitter),
                            &data_hash,
                        );
                    });
                submitters
                    .iter_scope(&QuorumScope::Privileged)
                    .for_each(|submitter| {
                        crate::SubmissionsV1::<Test, Api>::insert(
                            (&batch_id, QuorumScope::Privileged, &submitter),
                            &data_hash,
                        );
                    });
            });

        // Give it enough weight do do exactly 8 iterations:
        let limit = <Test as pallet_migrations::Config>::WeightInfo::progress_mbms_none()
            + pallet_migrations::Pallet::<Test>::exec_migration_max_weight()
            + crate::weights::SubstrateWeight::<Test>::migration_v1_v2_step() * 8;
        MigratorServiceWeight::set(&limit);

        System::set_block_number(1);
        AllPalletsWithSystem::on_runtime_upgrade(); // onboard MBMs

        // check migration progress across many blocks
        let mut last_num_migrated = 0;
        for block in 2..=10 {
            run_to_block(block);

            let num_migrated = (0..64)
                .map(submissions_from_seed)
                .map(|(seeded_batch_id, _, _)| {
                    if crate::BatchQueue::<Test, Api>::iter()
                        .any(|(_, batch_id)| seeded_batch_id == batch_id)
                    {
                        1usize
                    } else {
                        0usize
                    }
                })
                .sum();

            // part of the first block of migration is completing lazy migration v1 (noop), but
            // this costs some weight so we cannot migrate 8 batches in the first block.
            let expected_migrated_number = if block == 2 || block == 10 { 4 } else { 8 };

            assert_eq!(num_migrated, last_num_migrated + expected_migrated_number);
            last_num_migrated = num_migrated;
        }

        // Check that everything is migrated now
        (0..64u64)
            .map(submissions_from_seed)
            .for_each(|(seeded_batch_id, _, _)| {
                assert!(crate::BatchQueue::<Test, Api>::iter()
                    .any(|(_, batch_id)| seeded_batch_id == batch_id))
            });

        assert_eq!(crate::BatchQueue::<Test, Api>::count(), 64);
    });
}
