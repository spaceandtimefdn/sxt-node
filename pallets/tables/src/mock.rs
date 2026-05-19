use std::cell::RefCell;

use codec::{Decode, Encode};
use polkadot_sdk::frame_support::derive_impl;
use polkadot_sdk::frame_support::traits::ConstU128;
use polkadot_sdk::sp_core::crypto::AccountId32;
use polkadot_sdk::sp_runtime::traits::IdentityLookup;
use polkadot_sdk::sp_runtime::BuildStorage;
use polkadot_sdk::{frame_support, frame_system, pallet_balances, sp_io};
use proof_of_sql_commitment_map::generic_over_commitment::ConcreteType;
use proof_of_sql_commitment_map::PerCommitmentScheme;
use sxt_core::prover_db_indexer::{BlockEvent, EventCapture};

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
        Balances: pallet_balances,
    }
);

type Balance = u128;

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountId = AccountId32;
    type Lookup = IdentityLookup<Self::AccountId>;
    type AccountData = pallet_balances::AccountData<Balance>;
}

impl pallet_tables::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type EventCapture = RecordingEventCapture;
}

thread_local! {
    static CAPTURED_EVENTS: RefCell<Vec<BlockEvent<'static>>> = const { RefCell::new(Vec::new()) };
}

/// Test-only `EventCapture` impl that records every call into a thread-local
/// buffer so tests can assert which `BlockEvent`s the pallet emitted. Roundtrips
/// the input through SCALE so the stored events own their fields (`Cow::Owned`)
/// and don't borrow from the caller's stack.
pub struct RecordingEventCapture;

impl EventCapture for RecordingEventCapture {
    fn capture_events(events: Vec<BlockEvent<'_>>) {
        let bytes = events.encode();
        let owned = Vec::<BlockEvent<'static>>::decode(&mut &bytes[..])
            .expect("encode/decode roundtrip is infallible");
        CAPTURED_EVENTS.with(|c| c.borrow_mut().extend(owned));
    }
}

/// Drain and return every event captured since the last call (or test start).
/// Tests should call this *after* the action under test.
pub fn drain_captured_events() -> Vec<BlockEvent<'static>> {
    CAPTURED_EVENTS.with(|c| std::mem::take(&mut *c.borrow_mut()))
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

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
    type Balance = Balance;
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposit = ConstU128<1>;
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    // Clear any captures from a previous test that may have run on this
    // thread before us (cargo test reuses worker threads).
    CAPTURED_EVENTS.with(|c| c.borrow_mut().clear());
    frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap()
        .into()
}
