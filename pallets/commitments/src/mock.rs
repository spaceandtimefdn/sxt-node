use polkadot_sdk::frame_support::derive_impl;
use polkadot_sdk::frame_support::migrations::MultiStepMigrator;
use polkadot_sdk::frame_support::traits::{OnFinalize, OnInitialize};
use polkadot_sdk::sp_runtime::BuildStorage;
use polkadot_sdk::sp_weights::Weight;
use polkadot_sdk::{frame_support, frame_system, pallet_migrations, sp_io};
use proof_of_sql_commitment_map::generic_over_commitment::ConcreteType;
use proof_of_sql_commitment_map::PerCommitmentScheme;
use proof_of_sql_static_setups::io::get_or_init_from_files_with_four_points_unchecked;

use crate as pallet_commitments;

type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        CommitmentsModule: pallet_commitments,
        Migrator: pallet_migrations,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type MultiBlockMigrator = Migrator;
}

impl pallet_commitments::Config for Test {
    const END_ROW_LIMITS_PER_SCHEME: PerCommitmentScheme<ConcreteType<u32>> = PerCommitmentScheme {
        hyper_kzg: 4,
        dynamic_dory: 3,
    };
}

frame_support::parameter_types! {
    pub storage MigratorServiceWeight: Weight = Weight::from_parts(100, 100); // do not use in prod
}

#[derive_impl(pallet_migrations::config_preludes::TestDefaultConfig)]
impl pallet_migrations::Config for Test {
    #[cfg(not(feature = "runtime-benchmarks"))]
    type Migrations = (
        crate::migrations::delete_dynamic_dory::DeleteDynamicDoryCommitmentsLazyMigration<
            Test,
            crate::migrations::delete_dynamic_dory::weights::SubstrateWeight<Test>,
        >,
    );
    #[cfg(feature = "runtime-benchmarks")]
    type Migrations = pallet_migrations::mock_helpers::MockedMigrations;
    type MaxServiceWeight = MigratorServiceWeight;
}

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

#[allow(dead_code)]
pub fn run_to_block(n: u64) {
    assert!(System::block_number() < n);
    while System::block_number() < n {
        let b = System::block_number();
        AllPalletsWithSystem::on_finalize(b);
        // Done by Executive:
        <Test as frame_system::Config>::MultiBlockMigrator::step();
        System::set_block_number(b + 1);
        AllPalletsWithSystem::on_initialize(b + 1);
    }
}
