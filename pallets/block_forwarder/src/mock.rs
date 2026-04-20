//! Mock runtime for testing the block forwarder pallet.

use frame_support::derive_impl;
use frame_support::traits::ConstU8;
use sp_core::H256;
use sp_runtime::traits::IdentityLookup;
use sp_runtime::BuildStorage;

use crate as pallet_block_forwarder;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Permissions: pallet_permissions,
        Commitments: pallet_commitments,
        Tables: pallet_tables,
        BlockForwarder: pallet_block_forwarder,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Nonce = u32;
    type AccountId = u64;
    type Block = Block;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Hash = H256;
}

impl pallet_permissions::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
}

impl pallet_commitments::Config for Test {}

impl pallet_tables::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
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
    let storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    storage.into()
}
