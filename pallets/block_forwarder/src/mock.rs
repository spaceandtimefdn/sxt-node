//! Mock runtime for testing the block forwarder pallet.

use polkadot_sdk::frame_support::derive_impl;
use polkadot_sdk::frame_support::traits::{ConstU8, ConstU128};
use polkadot_sdk::sp_core::crypto::AccountId32;
use polkadot_sdk::sp_runtime::traits::IdentityLookup;
use polkadot_sdk::sp_runtime::BuildStorage;
use polkadot_sdk::{frame_support, frame_system, pallet_balances, sp_io};
use proof_of_sql_commitment_map::generic_over_commitment::ConcreteType;
use proof_of_sql_commitment_map::PerCommitmentScheme;

use crate as pallet_block_forwarder;

type Block = frame_system::mocking::MockBlock<Test>;
type Balance = u128;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Permissions: pallet_permissions,
        Tables: pallet_tables,
        Commitments: pallet_commitments,
        Balances: pallet_balances,
        BlockForwarder: pallet_block_forwarder,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountId = AccountId32;
    type Lookup = IdentityLookup<Self::AccountId>;
    type AccountData = pallet_balances::AccountData<Balance>;
}

impl pallet_permissions::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
}

impl pallet_commitments::Config for Test {
    const END_ROW_LIMITS_PER_SCHEME: PerCommitmentScheme<ConcreteType<u32>> = PerCommitmentScheme {
        hyper_kzg: 4,
        dynamic_dory: 4,
    };
}

impl pallet_tables::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
    type Balance = Balance;
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposit = ConstU128<1>;
}

impl pallet_block_forwarder::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    // Tests don't exercise the indexing-event extraction path, so we stand
    // in with any pallet that's already in the mock runtime. The variant
    // index is arbitrary for the same reason.
    type IndexingPallet = Tables;
    type QuorumReachedVariantIndex = ConstU8<1>;
}

/// Build genesis storage for a test externalities.
pub fn new_test_ext() -> sp_io::TestExternalities {
    frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap()
        .into()
}
