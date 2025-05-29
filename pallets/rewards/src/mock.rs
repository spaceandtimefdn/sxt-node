use frame_election_provider_support::bounds::{ElectionBounds, ElectionBoundsBuilder};
use frame_election_provider_support::private::sp_arithmetic::FixedU128;
use frame_election_provider_support::{onchain, SequentialPhragmen};
use frame_support::pallet_prelude::ConstU32;
use frame_support::traits::{ConstU128, KeyOwnerProofSystem, VariantCountOf};
use frame_support::{derive_impl, parameter_types};
use pallet_grandpa::AuthorityId as GrandpaId;
use proof_of_sql_static_setups::io::get_or_init_from_files_with_four_points_unchecked;
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_core::{ConstU64, H256};
use sp_runtime::traits::{
    ConvertInto,
    IdentityLookup,
    MaybeConvert,
    OpaqueKeys,
    TryConvertInto,
    UniqueSaturatedInto,
    Zero,
};
use sp_runtime::{generic, BuildStorage, FixedPointNumber, KeyTypeId};

use crate as pallet_rewards;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Permissions: pallet_permissions,
        Commitments: pallet_commitments,
        Tables: pallet_tables,
        Session: pallet_session,
        Historical: pallet_session::historical,
        Rewards: pallet_rewards,
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

impl pallet_rewards::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type MaxPayoutsPerBlock = ConstU32<3>;
}

parameter_types! {
    pub EpochDuration: u64 = 50;
    pub const ExpectedBlockTime: u64 = 4000;
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
    type EventHandler = (Staking);
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

parameter_types! {
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

/// Defines how much should the inflation be for an era given its duration.
pub struct EraPayout;
impl pallet_staking::EraPayout<Balance> for EraPayout {
    fn era_payout(
        total_staked: Balance,
        total_issuance: Balance,
        era_duration_millis: u64,
    ) -> (Balance, Balance) {
        const MILLISECONDS_PER_YEAR: u64 = (1000 * 3600 * 24 * 36525) / 100;
        // A normal-sized era will have 1 / 365.25 here:
        let relative_era_len =
            FixedU128::from_rational(era_duration_millis.into(), MILLISECONDS_PER_YEAR.into());

        let base_rate = FixedU128::from_rational(79, 1000);
        let yearly_emission = base_rate.saturating_mul_int(total_staked);

        let era_emission = relative_era_len.saturating_mul_int(yearly_emission);

        (era_emission.unique_saturated_into(), Balance::zero())
    }
}

#[derive_impl(pallet_staking::config_preludes::TestDefaultConfig)]
impl pallet_staking::Config for Test {
    type Currency = Balances;
    type CurrencyBalance = <Self as pallet_balances::Config>::Balance;
    type UnixTime = pallet_timestamp::Pallet<Self>;
    type AdminOrigin = frame_system::EnsureRoot<Self::AccountId>;
    type SessionInterface = Self;
    type EraPayout = EraPayout;
    type NextNewSession = Session;
    type ElectionProvider = onchain::OnChainExecution<OnChainSeqPhragmen>;
    type GenesisElectionProvider = Self::ElectionProvider;
    type VoterList = pallet_staking::UseNominatorsAndValidatorsMap<Self>;
    type TargetList = pallet_staking::UseValidatorsMap<Self>;
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
            pub babe: Babe,
            pub grandpa: Grandpa,
            pub authority_discovery: AuthorityDiscovery,
    }
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
    let _ = get_or_init_from_files_with_four_points_unchecked();
    let mut storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_commitments::GenesisConfig::<Test>::default()
        .assimilate_storage(&mut storage)
        .unwrap();

    storage.into()
}
