// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

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
                v1::v0::Submissions::<Test, Api>::insert(batch_id, data_hash, submitters)
            });

        // Give it enough weight do do exactly 16 iterations:
        let limit = <Test as pallet_migrations::Config>::WeightInfo::progress_mbms_none()
            + pallet_migrations::Pallet::<Test>::exec_migration_max_weight()
            + crate::weights::SubstrateWeight::<Test>::migration_v0_v1_step() * 8;
        MigratorServiceWeight::set(&limit);

        System::set_block_number(1);
        AllPalletsWithSystem::on_runtime_upgrade(); // onboard MBMs

        // check migration progress across many blocks
        let mut last_num_migrated = 0;
        for block in 2..=9 {
            run_to_block(block);

            let mut num_migrated = 0;
            (0..64u64)
                .map(submissions_from_seed)
                .for_each(|(batch_id, data_hash, submitters)| {
                    let first_submitter = submitters
                        .iter_scope(&QuorumScope::Public)
                        .next()
                        .expect("seeding ensures at least one public submitter");
                    let is_migrated = crate::SubmissionsV1::<Test, Api>::get((
                        &batch_id,
                        QuorumScope::Public,
                        first_submitter,
                    ))
                    .is_some();

                    if is_migrated {
                        submitters
                            .iter_scope(&QuorumScope::Public)
                            .for_each(|submitter| {
                                assert_eq!(
                                    crate::SubmissionsV1::<Test, Api>::get((
                                        &batch_id,
                                        QuorumScope::Public,
                                        submitter
                                    )),
                                    Some(data_hash)
                                );
                            });
                        submitters
                            .iter_scope(&QuorumScope::Privileged)
                            .for_each(|submitter| {
                                assert_eq!(
                                    crate::SubmissionsV1::<Test, Api>::get((
                                        &batch_id,
                                        QuorumScope::Privileged,
                                        submitter
                                    )),
                                    Some(data_hash)
                                );
                            });
                        assert!(!v1::v0::Submissions::<Test, Api>::contains_key(
                            &batch_id, data_hash
                        ),);
                        num_migrated += 1
                    } else {
                        assert_eq!(
                            v1::v0::Submissions::<Test, Api>::get(&batch_id, data_hash),
                            submitters
                        );
                    }
                });

            assert_eq!(num_migrated, last_num_migrated + 8);
            last_num_migrated = num_migrated;
        }

        // Check that everything is migrated now
        (0..64u64)
            .map(submissions_from_seed)
            .for_each(|(batch_id, data_hash, submitters)| {
                submitters
                    .iter_scope(&QuorumScope::Public)
                    .for_each(|submitter| {
                        assert_eq!(
                            crate::SubmissionsV1::<Test, Api>::get((
                                &batch_id,
                                QuorumScope::Public,
                                submitter
                            )),
                            Some(data_hash)
                        );
                    });
                submitters
                    .iter_scope(&QuorumScope::Privileged)
                    .for_each(|submitter| {
                        assert_eq!(
                            crate::SubmissionsV1::<Test, Api>::get((
                                &batch_id,
                                QuorumScope::Privileged,
                                submitter
                            )),
                            Some(data_hash)
                        );
                    });
                assert!(!v1::v0::Submissions::<Test, Api>::contains_key(
                    &batch_id, data_hash
                ),);
            });
    });
}
