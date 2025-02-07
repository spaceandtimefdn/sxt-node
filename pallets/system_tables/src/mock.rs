use frame_support::pallet_prelude::ConstU32;
use frame_support::traits::{ConstU128, VariantCountOf};
use frame_support::{derive_impl, parameter_types};
use sp_core::H256;
use sp_runtime::testing::UintAuthorityId;
use sp_runtime::traits::{IdentifyAccount, Verify};
use sp_runtime::{BuildStorage, FixedU128, MultiSignature};

use crate as pallet_system_tables;

type Block = frame_system::mocking::MockBlock<Test>;
type Balance = u128;
type BlockNumber = u64;
pub type Signature = sp_runtime::MultiSignature;
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        SystemTables: pallet_system_tables, // Pallet under Test
        Balances: pallet_balances, // Required by Parsing
        Tables: pallet_tables, // Required by Parsing
        Permissions: pallet_permissions, // Required by Tables
        Staking: pallet_staking, // Required by Parsing
        Session: pallet_session, // Required by staking
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type Hash = H256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<AccountId>;
}

impl pallet_system_tables::Config for Test {
    type RuntimeEvent = RuntimeEvent;
}

parameter_types! {
    pub static SessionsPerEra: sp_staking::SessionIndex = 3;
    pub static ExistentialDeposit: Balance = 1;
    pub static SlashDeferDuration: sp_staking::EraIndex = 0;
    pub static Period: BlockNumber = 5;
    pub static Offset: BlockNumber = 0;
    pub static MaxControllersInDeprecationBatch: u32 = 5900;
}

pub struct OtherSessionHandler;
impl frame_support::traits::OneSessionHandler<AccountId> for OtherSessionHandler {
    type Key = UintAuthorityId;

    fn on_genesis_session<'a, I: 'a>(_: I)
    where
        I: Iterator<Item = (&'a AccountId, Self::Key)>,
        AccountId: 'a,
    {
    }

    fn on_new_session<'a, I: 'a>(_: bool, _: I, _: I)
    where
        I: Iterator<Item = (&'a AccountId, Self::Key)>,
        AccountId: 'a,
    {
    }

    fn on_disabled(_validator_index: u32) {}
}

sp_runtime::impl_opaque_keys! {
    pub struct SessionKeys {
        pub other: AccountId,
    }
}

impl pallet_session::Config for Test {
    type SessionManager = pallet_session::historical::NoteHistoricalRoot<Test, Staking>;
    type Keys = SessionKeys;
    type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
    type SessionHandler = (OtherSessionHandler,);
    type RuntimeEvent = RuntimeEvent;
    type ValidatorId = AccountId;
    type ValidatorIdOf = pallet_staking::StashOf<Test>;
    type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
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

impl pallet_balances::Config for Test {
    /// The ubiquitous event type.
    type RuntimeEvent = RuntimeEvent;
    type RuntimeHoldReason = ();
    type RuntimeFreezeReason = ();
    type WeightInfo = pallet_balances::weights::SubstrateWeight<Test>;
    /// The type for recording an account's balance.
    type Balance = Balance;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU128<100>;
    type AccountStore = System;
    type ReserveIdentifier = [u8; 8];
    type FreezeIdentifier = RuntimeFreezeReason;
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ();
    type MaxFreezes = VariantCountOf<RuntimeFreezeReason>;
}

/// Defines how much should the inflation be for an era given its duration.
pub struct EraPayout;
impl pallet_staking::EraPayout<Balance> for EraPayout {
    fn era_payout(
        _total_staked: Balance,
        _total_issuance: Balance,
        era_duration_millis: u64,
    ) -> (Balance, Balance) {
        // No payouts in testing right now
        (0, 0)
    }
}

type ElectionProvider = frame_election_provider_support::NoElection<(
    AccountId,
    BlockNumber,
    pallet_staking::Pallet<Test>,
    frame_support::traits::ConstU32<1000>,
)>;

impl pallet_staking::Config for Test {
    type Currency = Balances;
    type CurrencyBalance = <Self as pallet_balances::Config>::Balance;
    type UnixTime = ();
    type AdminOrigin = frame_system::EnsureRoot<Self::AccountId>;
    type SessionInterface = Self;
    type EraPayout = EraPayout;
    type NextNewSession = Session;
    type GenesisElectionProvider = Self::ElectionProvider;
    type VoterList = pallet_staking::UseNominatorsAndValidatorsMap<Self>;
    type TargetList = pallet_staking::UseValidatorsMap<Self>;
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
