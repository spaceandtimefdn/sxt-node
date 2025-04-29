use frame_support::derive_impl;
use proof_of_sql_static_setups::io::get_or_init_from_files_with_four_points_unchecked;
use sp_runtime::BuildStorage;

use crate as pallet_commitments;

type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        CommitmentsModule: pallet_commitments,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
}

impl pallet_commitments::Config for Test {}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let _ = get_or_init_from_files_with_four_points_unchecked();
    let mut storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_commitments::GenesisConfig::<Test>::default()
        .assimilate_storage(&mut storage)
        .unwrap();

    storage.into()
}
