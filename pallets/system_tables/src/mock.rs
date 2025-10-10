use frame_election_provider_support::bounds::{ElectionBounds, ElectionBoundsBuilder};
use frame_election_provider_support::{onchain, SequentialPhragmen};
use frame_support::pallet_prelude::ConstU32;
use frame_support::traits::{ConstU128, Hooks, KeyOwnerProofSystem};
use frame_support::{derive_impl, parameter_types};
use proof_of_sql_commitment_map::generic_over_commitment::ConcreteType;
use proof_of_sql_commitment_map::PerCommitmentScheme;
use proof_of_sql_static_setups::io::get_or_init_from_files_with_four_points_unchecked;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_core::{ConstU64, Get, H256};
use sp_runtime::traits::{IdentityLookup, OpaqueKeys, Zero};
use sp_runtime::{BuildStorage, KeyTypeId};
use sp_staking::{EraIndex, SessionIndex};

use crate as pallet_system_tables;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Timestamp: pallet_timestamp,
        Permissions: pallet_permissions,
        Commitments: pallet_commitments,
        Tables: pallet_tables,
        Session: pallet_session,
        ZkPay: pallet_zkpay,
        Historical: pallet_session::historical,
        SystemTables: pallet_system_tables,
        Balances: pallet_balances,
        Staking: pallet_staking,
        Babe: pallet_babe,
        Grandpa: pallet_grandpa,
        Authorship: pallet_authorship,
        AuthorityDiscovery: pallet_authority_discovery,
    }
);

type AccountId = sp_core::crypto::AccountId32;
type Nonce = u32;
type Balance = u128;

const INIT_TIMESTAMP: u64 = 4000;
const BLOCK_TIME: u64 = 4000;

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Nonce = Nonce;
    type AccountId = AccountId;
    type AccountData = pallet_balances::AccountData<Balance>;
    type RuntimeCall = RuntimeCall;

    type Block = Block;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Hash = H256;
}

pub const MAX_AUTHORITIES: u32 = 100_000u32;
pub const BLOCKS_PER_EPOCH: u64 = 50;
parameter_types! {
    pub EpochDuration: u64 = BLOCKS_PER_EPOCH;
    pub const ExpectedBlockTime: u64 = BLOCK_TIME;
    pub ReportLongevity: u64 = 100;
}
impl pallet_babe::Config for Test {
    type EpochDuration = EpochDuration;
    type ExpectedBlockTime = ExpectedBlockTime;
    type EpochChangeTrigger = pallet_babe::ExternalTrigger;
    type DisabledValidators = Session;
    type WeightInfo = ();
    type MaxAuthorities = ConstU32<MAX_AUTHORITIES>;
    type MaxNominators = ConstU32<100_000>;
    type KeyOwnerProof = <Historical as KeyOwnerProofSystem<(KeyTypeId, BabeId)>>::Proof;
    type EquivocationReportSystem = ();
}

impl pallet_authorship::Config for Test {
    type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Babe>;
    type EventHandler = Staking;
}

impl pallet_grandpa::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type MaxAuthorities = ConstU32<MAX_AUTHORITIES>;
    type MaxNominators = ConstU32<0>;
    type MaxSetIdSessionEntries = ConstU64<0>;
    type KeyOwnerProof = <Historical as KeyOwnerProofSystem<(KeyTypeId, BabeId)>>::Proof;
    type EquivocationReportSystem = ();
}
impl pallet_authority_discovery::Config for Test {
    type MaxAuthorities = ConstU32<MAX_AUTHORITIES>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
    type Balance = Balance;
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposit = ConstU128<1>;
}

impl pallet_timestamp::Config for Test {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = ConstU64<5>;
    type WeightInfo = ();
}

pallet_staking_reward_curve::build! {
    const I_NPOS: sp_runtime::curve::PiecewiseLinear<'static> = curve!(
        min_inflation: 0_025_000,
        max_inflation: 0_100_000,
        ideal_stake: 0_500_000,
        falloff: 0_050_000,
        max_piece_count: 40,
        test_precision: 0_005_000,
    );
}
parameter_types! {
    pub const RewardCurve: &'static sp_runtime::curve::PiecewiseLinear<'static> = &I_NPOS;
    pub static ElectionsBounds: ElectionBounds = ElectionBoundsBuilder::default().build();

    // Twenty-Four sessions in an era (24 hours).
    pub const SessionsPerEra: sp_staking::SessionIndex = 24;

    // 7 eras for unbonding (7 days).
    pub BondingDuration: sp_staking::EraIndex = 7;
}

pub struct OnChainSeqPhragmen;
impl onchain::Config for OnChainSeqPhragmen {
    type System = Test;
    type Solver = SequentialPhragmen<AccountId, sp_runtime::Perbill>;
    type DataProvider = Staking;
    type WeightInfo = ();
    type MaxWinners = ConstU32<100>;
    type Bounds = ElectionsBounds;
}

#[derive_impl(pallet_staking::config_preludes::TestDefaultConfig)]
impl pallet_staking::Config for Test {
    type Currency = Balances;
    type CurrencyBalance = <Self as pallet_balances::Config>::Balance;
    type UnixTime = pallet_timestamp::Pallet<Self>;
    type AdminOrigin = frame_system::EnsureRoot<Self::AccountId>;
    type SessionInterface = Self;
    type EraPayout = pallet_staking::ConvertCurve<RewardCurve>;
    type NextNewSession = Session;
    type ElectionProvider = onchain::OnChainExecution<OnChainSeqPhragmen>;
    type GenesisElectionProvider = Self::ElectionProvider;
    type VoterList = pallet_staking::UseNominatorsAndValidatorsMap<Self>;
    type TargetList = pallet_staking::UseValidatorsMap<Self>;
    type CurrencyToVote = sp_staking::currency_to_vote::U128CurrencyToVote;
    type NominationsQuota = pallet_staking::FixedNominationsQuota<16>;
    type RewardRemainder = (); // Reward Remainders are burned
    type RuntimeEvent = RuntimeEvent;
    type Slash = (); // Slashed funds will be burned
    type Reward = (); // Rewards are minted not transferred
    type MaxControllersInDeprecationBatch = ();
    type EventListeners = (); // This will be needed if we add nomination pools
    type DisablingStrategy = pallet_staking::UpToLimitDisablingStrategy;
    type WeightInfo = pallet_staking::weights::SubstrateWeight<Test>;
}

pub type BlockNumber = u64;

parameter_types! {
    /// Length of each session
    pub const Period: BlockNumber = BLOCKS_PER_EPOCH;
    /// Length of the very first session
    pub const Offset: BlockNumber = 0;
}

pub struct TestSessionHandler;
impl pallet_session::SessionHandler<AccountId> for TestSessionHandler {
    const KEY_TYPE_IDS: &'static [KeyTypeId] = &[sp_core::crypto::key_types::DUMMY];

    fn on_new_session<Ks: OpaqueKeys>(
        _changed: bool,
        _validators: &[(AccountId, Ks)],
        _queued_validators: &[(AccountId, Ks)],
    ) {
    }

    fn on_disabled(_validator_index: u32) {}

    fn on_genesis_session<Ks: OpaqueKeys>(_validators: &[(AccountId, Ks)]) {}
}

impl pallet_session::Config for Test {
    type SessionManager = ();
    type Keys = sp_runtime::testing::UintAuthorityId;
    type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
    type SessionHandler = TestSessionHandler;
    type RuntimeEvent = RuntimeEvent;
    type ValidatorId = AccountId;
    type ValidatorIdOf = pallet_staking::StashOf<Test>;
    type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
    type WeightInfo = ();
}
impl pallet_session::historical::Config for Test {
    type FullIdentification = pallet_staking::Exposure<AccountId, Balance>;
    type FullIdentificationOf = pallet_staking::ExposureOf<Test>;
}

sp_runtime::impl_opaque_keys! {
    pub struct SessionKeys {
        pub foo: sp_runtime::testing::UintAuthorityId,
    }
}

impl pallet_zkpay::Config for Test {
    type RuntimeEvent = RuntimeEvent;
}

impl pallet_system_tables::Config for Test {
    type RuntimeEvent = RuntimeEvent;
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

/// Progress to the given block, triggering session and era changes as we progress.
///
/// This will finalize the previous block, initialize up to the given block, essentially simulating
/// a block import/propose process where we first initialize the block, then execute some stuff (not
/// in the function), and then finalize the block.
pub(crate) fn run_to_block(n: BlockNumber) {
    finalize_block(n);
    for b in (System::block_number() + 1)..=n {
        System::set_block_number(b);
        initialize_block(b);
        let system_block = System::block_number();
        let time = system_block * BLOCK_TIME + INIT_TIMESTAMP;
        // The staking pallet depends on the time stamp to identify new eras
        Timestamp::set_timestamp(time);
        if b != n {
            finalize_block(System::block_number());
        }
    }
}

fn finalize_block(n: BlockNumber) {
    Session::on_finalize(n);
    Historical::on_finalize(n);
    Staking::on_finalize(n);
    Babe::on_finalize(n);
    Grandpa::on_finalize(n);
    Authorship::on_finalize(n);
    AuthorityDiscovery::on_finalize(n);
}

fn initialize_block(n: BlockNumber) {
    Session::on_initialize(n);
    Historical::on_initialize(n);
    <Staking as Hooks<u64>>::on_initialize(n);
    Babe::on_initialize(n);
    Grandpa::on_initialize(n);
    Authorship::on_initialize(n);
    AuthorityDiscovery::on_initialize(n);
}

/// Progresses from the current block number (whatever that may be) to the `P * session_index + 1`.
pub(crate) fn start_session(session_index: SessionIndex) {
    // Figure out which block number is the end of session just before the target session
    let end: u64 = if Offset::get().is_zero() {
        (session_index as u64) * Period::get()
    } else {
        Offset::get() + (session_index.saturating_sub(1) as u64) * Period::get()
    };

    // Run to that block, calling hooks as we go
    run_to_block(end);

    // Assert the session progressed properly.
    assert_eq!(
        Session::current_index(),
        session_index,
        "current session index = {}, expected = {}",
        Session::current_index(),
        session_index,
    );
}

/// Progress until the given era.
pub(crate) fn start_active_era(era_index: EraIndex) {
    start_session(era_index * <SessionsPerEra as Get<u32>>::get());
    pallet_staking::CurrentEra::<Test>::set(Some(era_index));
    pallet_staking::ActiveEra::<Test>::set(Some(pallet_staking::ActiveEraInfo {
        index: era_index,
        start: None,
    }));

    assert_eq!(active_era(), era_index);
    assert_eq!(current_era(), active_era());
}

pub(crate) fn active_era() -> EraIndex {
    Staking::active_era().unwrap().index
}

pub(crate) fn current_era() -> EraIndex {
    Staking::current_era().unwrap()
}
