use frame_election_provider_support::bounds::{ElectionBounds, ElectionBoundsBuilder};
use frame_election_provider_support::{onchain, SequentialPhragmen};
use frame_support::{derive_impl, parameter_types};
use native_api::Api;
use sp_core::{ConstU32, ConstU64, H256};
use sp_runtime::traits::{ConvertInto, IdentityLookup, MaybeConvert, OpaqueKeys, TryConvertInto};
use sp_runtime::{BuildStorage, KeyTypeId};

use crate as pallet_smartcontracts;

type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Permissions: pallet_permissions,
        Commitments: pallet_commitments,
        Tables: pallet_tables,
        Indexing: pallet_indexing::native_pallet,
        SmartContracts: pallet_smartcontracts::native_pallet,
        SystemTabales: pallet_system_tables,
        Session: pallet_session,
        Balances: pallet_balances,
        Staking: pallet_staking,
    }
);

type AccountId = u64;
type Nonce = u32;
type Balance = u128;

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Nonce = Nonce;
    type AccountId = AccountId;
    type AccountData = pallet_balances::AccountData<Balance>;
    type Block = Block;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Hash = H256;
}

impl pallet_balances::Config for Test {
    type AccountStore = System;
    type Balance = Balance;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeHoldReason = ();
    type RuntimeFreezeReason = ();
    type WeightInfo = ();
    type DustRemoval = ();
    type ExistentialDeposit = sp_core::ConstU128<1>;
    type ReserveIdentifier = ();
    type FreezeIdentifier = ();
    type MaxLocks = ();
    type MaxReserves = ();
    type MaxFreezes = ();
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

impl pallet_timestamp::Config for Test {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = ConstU64<5>;
    type WeightInfo = ();
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
    pub const Period: BlockNumber = 1;
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

impl pallet_smartcontracts::Config<Api> for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
}

impl pallet_permissions::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
}

impl pallet_tables::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
}

impl pallet_commitments::Config for Test {}

impl pallet_indexing::pallet::Config<Api> for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_indexing::weights::SubstrateWeight<Test>;
}

impl pallet_system_tables::Config for Test {
    type RuntimeEvent = RuntimeEvent;
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap()
        .into()
}
