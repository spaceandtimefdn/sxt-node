//! Mock runtime for testing the offchain indexing pallet.

use frame_support::derive_impl;
use sp_core::H256;
use sp_runtime::traits::IdentityLookup;
use sp_runtime::BuildStorage;

use crate as pallet_offchain_indexing;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Permissions: pallet_permissions,
        Commitments: pallet_commitments,
        Tables: pallet_tables,
        OffchainIndexing: pallet_offchain_indexing,
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

impl pallet_offchain_indexing::Config for Test {
    type RuntimeEvent = RuntimeEvent;
}

/// Build genesis storage for a test externalities.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    storage.into()
}
