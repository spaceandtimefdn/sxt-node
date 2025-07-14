use frame_support::derive_impl;
use proof_of_sql_commitment_map::generic_over_commitment::ConcreteType;
use proof_of_sql_commitment_map::PerCommitmentScheme;
use sp_runtime::BuildStorage;

use crate as pallet_tables;

type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Permissions: pallet_permissions,
        Tables: pallet_tables,
        Commitments: pallet_commitments,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
}

impl pallet_tables::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
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

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap()
        .into()
}
