//! Minimal mock runtime for testing the consumer (OCW HTTP forwarding).
//!
//! The pallet's `Config` only requires `frame_system::Config`, so the
//! mock is intentionally tiny — frame-system + the pallet itself. Tests
//! pre-populate the offchain DB with payload+high-water keys directly,
//! then drive `offchain_worker` and assert on outbound HTTP traffic.

use polkadot_sdk::frame_support::traits::ConstU64;
use polkadot_sdk::frame_support::{construct_runtime, derive_impl};
use polkadot_sdk::sp_core::H256;
use polkadot_sdk::sp_runtime::traits::IdentityLookup;
use polkadot_sdk::sp_runtime::BuildStorage;
use polkadot_sdk::{frame_system, sp_io, sp_runtime};

use crate as pallet_prover_db_indexer;

type Block = frame_system::mocking::MockBlock<Test>;

construct_runtime!(
    pub enum Test {
        System: frame_system,
        ProverDbIndexer: pallet_prover_db_indexer,
    }
);

type AccountId = sp_runtime::AccountId32;
type Nonce = u32;

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Nonce = Nonce;
    type AccountId = AccountId;
    type Block = Block;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Hash = H256;
}

impl pallet_prover_db_indexer::Config for Test {
    // Match runtime defaults so tests exercise the production limits.
    type MaxBlocksPerInvocation = ConstU64<100>;
    type OcwLockDeadlineMs = ConstU64<120_000>;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap()
        .into()
}
