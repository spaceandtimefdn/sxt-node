use frame_support::derive_impl;
use native_api::Api;
use sp_core::H256;
use sp_runtime::BuildStorage;

use crate as pallet_indexing;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Indexing: pallet_indexing::native_pallet,
        Permissions: pallet_permissions,
        Commitments: pallet_commitments,
        Tables: pallet_tables,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type Hash = H256;
}

impl pallet_indexing::pallet::Config<Api> for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_indexing::weights::SubstrateWeight<Test>;
}

impl pallet_tables::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
}

impl pallet_permissions::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
}

impl pallet_commitments::Config for Test {}

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_commitments::GenesisConfig::<Test>::default()
        .assimilate_storage(&mut storage)
        .unwrap();

    storage.into()
}
