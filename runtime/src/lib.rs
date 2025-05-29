#![allow(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]
// runtime construction via `frame_support::runtime` does a lot of recursion and requires us to increase the limit.
#![recursion_limit = "512"]

mod tests;

#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

use frame_election_provider_support::{generate_solution_type, onchain, SequentialPhragmen};
use frame_support::dispatch::DispatchClass;
use frame_support::genesis_builder_helper::{build_state, get_preset};
use frame_support::traits::VariantCountOf;
pub use frame_support::traits::{
    ConstBool,
    ConstU128,
    ConstU32,
    ConstU64,
    ConstU8,
    Currency,
    KeyOwnerProofSystem,
    Randomness,
    StorageInfo,
};
pub use frame_support::weights::constants::{
    BlockExecutionWeight,
    ExtrinsicBaseWeight,
    RocksDbWeight,
    WEIGHT_REF_TIME_PER_MILLIS,
};
use frame_support::weights::ConstantMultiplier;
pub use frame_support::weights::{IdentityFee, Weight};
pub use frame_support::{construct_runtime, derive_impl, parameter_types, StorageValue};
pub use frame_system::Call as SystemCall;
use frame_system::EnsureRoot;
pub use pallet_balances::Call as BalancesCall;
use pallet_election_provider_multi_phase::GeometricDepositBase;
use pallet_grandpa::AuthorityId as GrandpaId;
pub use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
pub use pallet_timestamp::Call as TimestampCall;
use pallet_transaction_payment::{CurrencyAdapter, Multiplier};
use proof_of_sql_commitment_map::generic_over_commitment::ConcreteType;
use proof_of_sql_commitment_map::{AnyCommitmentScheme, TableCommitmentBytes};
use sp_api::impl_runtime_apis;
use sp_arithmetic::traits::UniqueSaturatedInto;
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_core::crypto::{AccountId32, KeyTypeId};
use sp_core::OpaqueMetadata;
use sp_runtime::traits::{
    AccountIdLookup,
    BlakeTwo256,
    Block as BlockT,
    Bounded,
    IdentifyAccount,
    NumberFor,
    One,
    OpaqueKeys,
    Verify,
    Zero,
};
use sp_runtime::transaction_validity::{
    TransactionPriority,
    TransactionSource,
    TransactionValidity,
};
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
use sp_runtime::{
    create_runtime_str,
    generic,
    impl_opaque_keys,
    ApplyExtrinsicResult,
    FixedPointNumber,
    FixedU128,
    MultiSignature,
    Perquintill,
};
pub use sp_runtime::{Perbill, Percent, Permill};
use sp_staking::SessionIndex;
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;
pub use {
    pallet_attestation,
    pallet_authority_discovery,
    pallet_authorship,
    pallet_babe,
    pallet_commitments,
    pallet_election_provider_multi_phase,
    pallet_grandpa,
    pallet_im_online,
    pallet_indexing,
    pallet_keystore,
    pallet_offences,
    pallet_permissions,
    pallet_rewards,
    pallet_session,
    pallet_smartcontracts,
    pallet_staking,
    pallet_system_contracts,
    pallet_system_tables,
    pallet_tables,
};

/// An index to a block.
pub type BlockNumber = u32;

/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// Balance of an account.
pub type Balance = u128;

/// Index of a transaction in the chain.
pub type Nonce = u32;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core data structures.
pub mod opaque {
    use sp_runtime::OpaqueExtrinsic;

    use super::*;

    /// Opaque block header type.
    pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
    /// Opaque block type.
    pub type Block = generic::Block<Header, OpaqueExtrinsic>;
    /// Opaque block identifier type.
    pub type BlockId = generic::BlockId<Block>;

    impl_opaque_keys! {
        pub struct SessionKeys {
            pub babe: Babe,
            pub grandpa: Grandpa,
            pub authority_discovery: AuthorityDiscovery,
            pub im_online: ImOnline,
        }
    }
}

// To learn more about runtime versioning, see:
// https://docs.substrate.io/main-docs/build/upgrade#runtime-versioning
#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: create_runtime_str!("sxt-runtime"),
    impl_name: create_runtime_str!("sxt-runtime"),
    authoring_version: 1,
    // The version of the runtime specification. A full node will not attempt to use its native
    //   runtime in substitute for the on-chain Wasm runtime unless all of `spec_name`,
    //   `spec_version`, and `authoring_version` are the same between Wasm and native.
    // This value is set to 100 to notify Polkadot-JS App (https://polkadot.js.org/apps) to use
    //   the compatible custom types.
    spec_version: 228,
    impl_version: 1,
    apis: RUNTIME_API_VERSIONS,
    transaction_version: 1,
    state_version: 1,
};

macro_rules! prod_or_dev {
    ($prod: expr, $dev: expr) => {
        if cfg!(feature = "fast-runtime") {
            $dev
        } else {
            $prod
        }
    };
}

/// Since BABE is probabilistic this is the average expected block time that
/// we are targeting. Blocks will be produced at a minimum duration defined
/// by `SLOT_DURATION`, but some slots will not be allocated to any
/// authority and hence no block will be produced. We expect to have this
/// block time on average following the defined slot duration and the value
/// of `c` configured for BABE (where `1 - c` represents the probability of
/// a slot being empty).
/// This value is only used indirectly to define the unit constants below
/// that are expressed in blocks. The rest of the code should use
/// `SLOT_DURATION` instead (like the Timestamp pallet for calculating the
/// minimum period).
///
/// If using BABE with secondary slots (default) then all of the slots will
/// always be assigned, in which case `MILLISECS_PER_BLOCK` and
/// `SLOT_DURATION` should have the same value.
///
/// <https://research.web3.foundation/Polkadot/protocols/block-production/Babe#6-practical-results>
pub const MILLISECS_PER_BLOCK: u64 = 3000;
pub const SECS_PER_BLOCK: u64 = MILLISECS_PER_BLOCK / 1000;

// NOTE: Currently it is not possible to change the slot duration after the chain has started.
//       Attempting to do so will brick block production.
pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;

// 1 in 4 blocks (on average, not counting collisions) will be primary BABE blocks.
pub const PRIMARY_PROBABILITY: (u64, u64) = (1, 4);
/// The BABE epoch configuration at genesis.
pub const BABE_GENESIS_EPOCH_CONFIG: sp_consensus_babe::BabeEpochConfiguration =
    sp_consensus_babe::BabeEpochConfiguration {
        c: PRIMARY_PROBABILITY,
        allowed_slots: sp_consensus_babe::AllowedSlots::PrimaryAndSecondaryVRFSlots,
    };

// Set the Block Length to a maximum of 15 Mebibytes
pub const MAX_BLOCK_SIZE: u32 = 15 * 1024 * 1024;

// These time units are defined in number of blocks;
pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
pub const HOURS: BlockNumber = MINUTES * 60;
pub const DAYS: BlockNumber = HOURS * 24;

pub const MAX_AUTHORITIES: u32 = 100_000u32;

/// Each epoch is 1 hour
pub const EPOCH_DURATION_IN_BLOCKS: u32 = prod_or_dev!(HOURS, MINUTES);

/// The version information used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
    NativeVersion {
        runtime_version: VERSION,
        can_author_with: Default::default(),
    }
}

const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

parameter_types! {
    pub const BlockHashCount: BlockNumber = 2400;
    pub const Version: RuntimeVersion = VERSION;
    /// We allow for 2.25 seconds of compute with a 3 second average block time.
    pub BlockWeights: frame_system::limits::BlockWeights =
        frame_system::limits::BlockWeights::with_sensible_defaults(
            Weight::from_parts(2_250u64 * WEIGHT_REF_TIME_PER_MILLIS, u64::MAX),
            NORMAL_DISPATCH_RATIO,
        );
    pub BlockLength: frame_system::limits::BlockLength = frame_system::limits::BlockLength
        ::max_with_normal_ratio(MAX_BLOCK_SIZE, NORMAL_DISPATCH_RATIO);
    pub const SS58Prefix: u8 = 42;
}

impl frame_system::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type BaseCallFilter = frame_support::traits::Everything;
    /// Block & extrinsics weights: base values and limits.
    type BlockWeights = BlockWeights;
    /// The maximum length of a block (in bytes).
    type BlockLength = BlockLength;
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type RuntimeTask = RuntimeTask;
    /// The type for storing how many extrinsics an account has signed.
    type Nonce = Nonce;
    /// The type for hashing blocks and tries.
    type Hash = Hash;
    type Hashing = BlakeTwo256;
    /// The identifier used to distinguish between accounts.
    type AccountId = AccountId;
    type Lookup = AccountIdLookup<AccountId, ()>;
    /// The block type for the runtime.
    type Block = Block;
    /// Maximum number of block number to block hash mappings to keep (oldest pruned first).
    type BlockHashCount = BlockHashCount;
    /// The weight of database operations that the runtime can invoke.
    type DbWeight = RocksDbWeight;
    /// Version of the runtime.
    type Version = Version;
    type PalletInfo = PalletInfo;
    /// The data to be stored in an account.
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = frame_system::weights::SubstrateWeight<Runtime>;
    /// This is used as an identifier of the chain. 42 is the generic substrate prefix.
    type SS58Prefix = SS58Prefix;
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<16>;
    type SingleBlockMigrations = ();
    type MultiBlockMigrator = MultiBlockMigrations;
    type PreInherents = ();
    type PostInherents = ();
    type PostTransactions = ();
}

impl frame_system::offchain::SigningTypes for Runtime {
    type Public = <Signature as Verify>::Signer;
    type Signature = Signature;
}
impl<C> frame_system::offchain::SendTransactionTypes<C> for Runtime
where
    RuntimeCall: From<C>,
{
    type Extrinsic = UncheckedExtrinsic;
    type OverarchingCall = RuntimeCall;
}

parameter_types! {
    pub StatementCost: Balance = DOLLARS;
    pub StatementByteCost: Balance = 100 * MILLICENTS;
    pub const MinAllowedStatements: u32 = 4;
    pub const MaxAllowedStatements: u32 = 10;
    pub const MinAllowedBytes: u32 = 1024;
    pub const MaxAllowedBytes: u32 = 4096;
}

impl pallet_statement::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type StatementCost = StatementCost;
    type ByteCost = StatementByteCost;
    type MinAllowedStatements = MinAllowedStatements;
    type MaxAllowedStatements = MaxAllowedStatements;
    type MinAllowedBytes = MinAllowedBytes;
    type MaxAllowedBytes = MaxAllowedBytes;
}

parameter_types! {
    pub MbmServiceWeight: Weight = Perbill::from_percent(80) * BlockWeights::get().max_block;
}

impl pallet_migrations::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    #[cfg(not(feature = "runtime-benchmarks"))]
    type Migrations = (
        pallet_indexing::migrations::v1::LazyMigrationV1<
            Runtime,
            pallet_indexing::SubstrateWeight<Runtime>,
            native_api::Api,
        >,
    );
    // Benchmarks need mocked migrations to guarantee that they succeed.
    #[cfg(feature = "runtime-benchmarks")]
    type Migrations = pallet_migrations::mock_helpers::MockedMigrations;
    type CursorMaxLen = ConstU32<65_536>;
    type IdentifierMaxLen = ConstU32<256>;
    type MigrationStatusHandler = ();
    type FailedMigrationHandler = frame_support::migrations::FreezeChainOnFailedMigration;
    type MaxServiceWeight = MbmServiceWeight;
    type WeightInfo = pallet_migrations::weights::SubstrateWeight<Runtime>;
}

impl pallet_utility::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type PalletsOrigin = OriginCaller;
    type WeightInfo = pallet_utility::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const TransactionByteFee: Balance = 100_000;
    pub const OperationalFeeMultiplier: u8 = 5;
    pub const TargetBlockFullness: Perquintill = Perquintill::from_percent(80);
    pub AdjustmentVariable: Multiplier = Multiplier::saturating_from_rational(1, 100_000);
    pub MinimumMultiplier: Multiplier = Multiplier::saturating_from_rational(1, 1_000_000_000u128);
    pub MaximumMultiplier: Multiplier = Bounded::max_value();
}
impl pallet_transaction_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnChargeTransaction = CurrencyAdapter<Balances, ()>;
    type WeightToFee = IdentityFee<Balance>;
    type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
    type FeeMultiplierUpdate = ();
    type OperationalFeeMultiplier = OperationalFeeMultiplier;
}

impl pallet_grandpa::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type MaxAuthorities = ConstU32<MAX_AUTHORITIES>;
    type MaxNominators = ConstU32<0>;
    type MaxSetIdSessionEntries = ConstU64<0>;
    type KeyOwnerProof = <Historical as KeyOwnerProofSystem<(KeyTypeId, GrandpaId)>>::Proof;
    type EquivocationReportSystem =
        pallet_grandpa::EquivocationReportSystem<Self, Offences, Historical, ReportLongevity>;
}

impl pallet_timestamp::Config for Runtime {
    /// A timestamp: milliseconds since the unix epoch.
    type Moment = u64;
    type OnTimestampSet = Babe;
    type MinimumPeriod = ConstU64<{ SLOT_DURATION / 2 }>;
    type WeightInfo = pallet_timestamp::weights::SubstrateWeight<Runtime>;
}

/// Existential deposit.
pub const EXISTENTIAL_DEPOSIT: Balance = 100;

pub const UNITS: Balance = 1_000_000_000_000_000_000;
pub const DOLLARS: Balance = UNITS; // 10_000_000_000
pub const GRAND: Balance = DOLLARS * 1_000; // 10_000_000_000_000
pub const CENTS: Balance = DOLLARS / 100; // 100_000_000
pub const MILLICENTS: Balance = CENTS / 1_000; // 100_000

pub const fn deposit(items: u32, bytes: u32) -> Balance {
    items as Balance * MILLICENTS + (bytes as Balance) * 100 * MILLICENTS
}

impl pallet_balances::Config for Runtime {
    /// The ubiquitous event type.
    type RuntimeEvent = RuntimeEvent;
    type RuntimeHoldReason = RuntimeHoldReason;
    type RuntimeFreezeReason = RuntimeHoldReason;
    type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
    /// The type for recording an account's balance.
    type Balance = Balance;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU128<EXISTENTIAL_DEPOSIT>;
    type AccountStore = System;
    type ReserveIdentifier = [u8; 8];
    type FreezeIdentifier = RuntimeFreezeReason;
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ();
    type MaxFreezes = VariantCountOf<RuntimeFreezeReason>;
}

impl pallet_sudo::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = pallet_sudo::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    // One storage item; key size is 32; value is size 4+4+16+32 bytes = 56 bytes.
    pub const DepositBase: Balance = deposit(1, 88);
    // Additional storage item size of 32 bytes.
    pub const DepositFactor: Balance = deposit(0, 32);
}

impl pallet_multisig::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type Currency = Balances;
    type DepositBase = DepositBase;
    type DepositFactor = DepositFactor;
    type MaxSignatories = ConstU32<100>;
    type WeightInfo = pallet_multisig::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub EpochDuration: u64 = EPOCH_DURATION_IN_BLOCKS as u64;
    pub const ExpectedBlockTime: u64 = MILLISECS_PER_BLOCK;
    pub ReportLongevity: u64 =
        BondingDuration::get() as u64 * SessionsPerEra::get() as u64 * EpochDuration::get();
}
impl pallet_babe::Config for Runtime {
    type EpochDuration = EpochDuration;
    type ExpectedBlockTime = ExpectedBlockTime;
    type EpochChangeTrigger = pallet_babe::ExternalTrigger;
    type DisabledValidators = Session;
    type WeightInfo = ();
    type MaxAuthorities = ConstU32<MAX_AUTHORITIES>;
    type MaxNominators = ConstU32<100_000>;
    type KeyOwnerProof = <Historical as KeyOwnerProofSystem<(KeyTypeId, BabeId)>>::Proof;
    type EquivocationReportSystem =
        pallet_babe::EquivocationReportSystem<Self, Offences, Historical, ReportLongevity>;
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

parameter_types! {
    // Twenty-Four sessions in an era (24 hours).
    pub const SessionsPerEra: SessionIndex = 24;

    // 7 eras for unbonding (7 days).
    pub BondingDuration: sp_staking::EraIndex = 7;
    pub SlashDeferDuration: sp_staking::EraIndex = 6;
    pub const MaxExposurePageSize: u32 = 512;
    pub const MaxNominators: u32 = 100_000;
    pub const OffendingValidatorsThreshold: Perbill = Perbill::from_percent(17);
    pub const MaxNominations: u32 = <NposCompactSolution16 as frame_election_provider_support::NposSolution>::LIMIT as u32;
    pub const MaxControllersInDeprecationBatch: u32 = 5900;
    pub HistoryDepth: u32 = 84;
}

/// Upper limit on the number of NPOS nominations.
const MAX_QUOTA_NOMINATIONS: u32 = 16;
impl pallet_staking::Config for Runtime {
    type Currency = Balances;
    type CurrencyBalance = Balance;
    type UnixTime = Timestamp;
    type CurrencyToVote = sp_staking::currency_to_vote::U128CurrencyToVote;
    type ElectionProvider =
        frame_election_provider_support::onchain::OnChainExecution<OnChainSeqPhragmen>;
    type GenesisElectionProvider = Self::ElectionProvider;
    type NominationsQuota = pallet_staking::FixedNominationsQuota<MAX_QUOTA_NOMINATIONS>;
    type HistoryDepth = HistoryDepth;
    type RewardRemainder = (); // Reward Remainders are burned
    type RuntimeEvent = RuntimeEvent;
    type Slash = (); // Slashed funds will be burned
    type Reward = (); // Rewards are minted not transfered
    type SessionsPerEra = SessionsPerEra;
    type BondingDuration = BondingDuration;
    type SlashDeferDuration = SlashDeferDuration;
    type AdminOrigin = frame_system::EnsureRoot<Self::AccountId>; // Admin is sudo
    type SessionInterface = Self;
    type EraPayout = EraPayout;
    type NextNewSession = Session;
    type MaxExposurePageSize = MaxExposurePageSize;
    type VoterList = VoterList;
    type TargetList = pallet_staking::UseValidatorsMap<Self>;
    type MaxUnlockingChunks = frame_support::traits::ConstU32<32>;
    type MaxControllersInDeprecationBatch = ();
    type EventListeners = (); // This will be needed if we add nomination pools
    type DisablingStrategy = pallet_staking::UpToLimitDisablingStrategy;
    type BenchmarkingConfig = StakingBenchmarkingConfig;
    type WeightInfo = pallet_staking::weights::SubstrateWeight<Runtime>;
}

impl pallet_authorship::Config for Runtime {
    type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Babe>;

    type EventHandler = (Staking, ImOnline);
}

parameter_types! {
    pub const ImOnlineUnsignedPriority: TransactionPriority = TransactionPriority::MAX;
    pub const MaxKeys: u32 = 10_000;
    pub const MaxPeerInHeartbeats: u32 = 10_000;
}
impl pallet_im_online::Config for Runtime {
    type AuthorityId = ImOnlineId;
    type MaxKeys = MaxKeys;
    type MaxPeerInHeartbeats = MaxPeerInHeartbeats;
    type RuntimeEvent = RuntimeEvent;
    type ValidatorSet = Historical;
    type NextSessionRotation = Babe;
    type ReportUnresponsiveness = Offences;
    type UnsignedPriority = ImOnlineUnsignedPriority;
    type WeightInfo = pallet_im_online::weights::SubstrateWeight<Runtime>;
}

impl pallet_offences::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type IdentificationTuple = pallet_session::historical::IdentificationTuple<Self>;
    type OnOffenceHandler = pallet_system_tables::ChillingOffenceHandler<Runtime>;
}

pub type OnChainAccuracy = sp_runtime::Perbill;
parameter_types! {
    // phase durations. 1/4 of the last session for each.
    // in testing: 1min or half of the session for each
    pub SignedPhase: u32 = EPOCH_DURATION_IN_BLOCKS / 4;
    pub UnsignedPhase: u32 = EPOCH_DURATION_IN_BLOCKS / 4;

    // signed config
    pub const SignedMaxSubmissions: u32 = 16;
    pub const SignedMaxRefunds: u32 = 16 / 4;
    pub const SignedFixedDeposit: Balance = deposit(2, 0);
    pub const SignedDepositIncreaseFactor: Percent = Percent::from_percent(10);
    // 0.01 DOT per KB of solution data.
    pub const SignedDepositByte: Balance = deposit(0, 10) / 1024;
    // Each good submission will get 1 DOT as reward
    pub SignedRewardBase: Balance = UNITS;

    // 1 hour session, 15 minute unsigned phase, 32 offchain executions.
    pub OffchainRepeat: BlockNumber = UnsignedPhase::get() / 32;

    pub const MaxElectingVoters: u32 = 22_500;
    /// We take the top 22500 nominators as electing voters and all of the validators as electable
    /// targets. Whilst this is the case, we cannot and shall not increase the size of the
    /// validator intentions.
    pub ElectionBounds: frame_election_provider_support::bounds::ElectionBounds =
        frame_election_provider_support::bounds::ElectionBoundsBuilder::default().voters_count(MaxElectingVoters::get().into()).build();
    /// Setup election pallet to support maximum winners upto 1200. This will mean Staking Pallet
    /// cannot have active validators higher than this count.
    pub const MaxActiveValidators: u32 = 1200;
}

generate_solution_type!(
    #[compact]
    pub struct NposCompactSolution16::<
        VoterIndex = u32,
        TargetIndex = u16,
        Accuracy = sp_runtime::PerU16,
        MaxVoters = MaxElectingVoters,
    >(16)
);

/// An OnChain Election Solver for Fallback operation
pub struct OnChainSeqPhragmen;
impl frame_election_provider_support::onchain::Config for OnChainSeqPhragmen {
    type System = Runtime;
    type Solver = SequentialPhragmen<AccountId, OnChainAccuracy>;
    type DataProvider = Staking;
    type WeightInfo = frame_election_provider_support::weights::SubstrateWeight<Runtime>;
    type MaxWinners = MaxActiveValidators;
    type Bounds = ElectionBounds;
}

pub struct StakingBenchmarkingConfig;
impl pallet_staking::BenchmarkingConfig for StakingBenchmarkingConfig {
    type MaxValidators = ConstU32<50>;
    type MaxNominators = ConstU32<10_000>;
}

parameter_types! {
    /// A limit for off-chain phragmen unsigned solution submission.
    ///
    /// We want to keep it as high as possible, but can't risk having it reject,
    /// so we always subtract the base block execution weight.
    pub OffchainSolutionWeightLimit: Weight = BlockWeights::get()
        .get(DispatchClass::Normal)
        .max_extrinsic
        .expect("Normal extrinsics have weight limit configured by default; qed")
        .saturating_sub(BlockExecutionWeight::get());

    /// A limit for off-chain phragmen unsigned solution length.
    ///
    /// We allow up to 90% of the block's size to be consumed by the solution.
    pub OffchainSolutionLengthLimit: u32 = Perbill::from_rational(90_u32, 100) *
        *BlockLength::get()
        .max
        .get(DispatchClass::Normal);
}

impl pallet_election_provider_multi_phase::MinerConfig for Runtime {
    type AccountId = AccountId;
    type Solution = NposCompactSolution16;
    type MaxVotesPerVoter = <
    <Self as pallet_election_provider_multi_phase::Config>::DataProvider
    as
    frame_election_provider_support::ElectionDataProvider
    >::MaxVotesPerVoter;
    type MaxLength = OffchainSolutionLengthLimit;
    type MaxWeight = OffchainSolutionWeightLimit;
    type MaxWinners = MaxActiveValidators;

    // The unsigned submissions have to respect the weight of the submit_unsigned call, thus their
    // weight estimate function is wired to this call's weight.
    fn solution_weight(v: u32, t: u32, a: u32, d: u32) -> Weight {
        <
        <Self as pallet_election_provider_multi_phase::Config>::WeightInfo
        as
        pallet_election_provider_multi_phase::WeightInfo
        >::submit_unsigned(v, t, a, d)
    }
}

impl pallet_authority_discovery::Config for Runtime {
    type MaxAuthorities = ConstU32<MAX_AUTHORITIES>;
}

/// The numbers configured here could always be more than the the maximum limits of staking pallet
/// to ensure election snapshot will not run out of memory. For now, we set them to smaller values
/// since the staking is bounded and the weight pipeline takes hours for this single pallet.
pub struct ElectionBenchmarkConfig;
impl pallet_election_provider_multi_phase::BenchmarkingConfig for ElectionBenchmarkConfig {
    const VOTERS: [u32; 2] = [1000, 2000];
    const TARGETS: [u32; 2] = [500, 1000];
    const ACTIVE_VOTERS: [u32; 2] = [500, 800];
    const DESIRED_TARGETS: [u32; 2] = [200, 400];
    const SNAPSHOT_MAXIMUM_VOTERS: u32 = 1000;
    const MINER_MAXIMUM_VOTERS: u32 = 1000;
    const MAXIMUM_TARGETS: u32 = 300;
}

parameter_types! {
    pub NposSolutionPriority: TransactionPriority =
        Perbill::from_percent(90) * TransactionPriority::MAX;

    pub MaxSetIdSessionEntries: u32 = BondingDuration::get() * SessionsPerEra::get();
}
impl pallet_election_provider_multi_phase::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type EstimateCallFee = TransactionPayment;
    type UnsignedPhase = UnsignedPhase;
    type SignedPhase = SignedPhase;
    type BetterSignedThreshold = ();
    type OffchainRepeat = OffchainRepeat;
    type MinerTxPriority = NposSolutionPriority;
    type MinerConfig = Self;
    type SignedMaxSubmissions = SignedMaxSubmissions;
    type SignedMaxWeight =
        <Self::MinerConfig as pallet_election_provider_multi_phase::MinerConfig>::MaxWeight;
    type SignedMaxRefunds = SignedMaxRefunds;
    type SignedRewardBase = SignedRewardBase;
    type SignedDepositByte = SignedDepositByte;
    type SignedDepositWeight = ();
    type MaxWinners = MaxActiveValidators;
    type SignedDepositBase =
        GeometricDepositBase<Balance, SignedFixedDeposit, SignedDepositIncreaseFactor>;
    type ElectionBounds = ElectionBounds;
    type SlashHandler = (); // Slashed amounts will be burned since we don't do anything special here
    type RewardHandler = (); // No additional action on rewards
    type DataProvider = Staking;
    type Fallback = frame_election_provider_support::NoElection<(
        AccountId,
        BlockNumber,
        Staking,
        MaxActiveValidators,
    )>;
    type GovernanceFallback = onchain::OnChainExecution<OnChainSeqPhragmen>;
    type Solver = SequentialPhragmen<
        AccountId,
        pallet_election_provider_multi_phase::SolutionAccuracyOf<Self>,
        (),
    >;
    type ForceOrigin = EnsureRoot<Self::AccountId>;
    type BenchmarkingConfig = ElectionBenchmarkConfig;
    type WeightInfo = pallet_election_provider_multi_phase::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const Period: u32 = 60 * MINUTES;
    pub const Offset: u32 = 0;
}

impl pallet_session::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ValidatorId = <Self as frame_system::Config>::AccountId;
    type ValidatorIdOf = pallet_staking::StashOf<Self>;
    type ShouldEndSession = Babe;
    type NextSessionRotation = Babe;
    type SessionManager = pallet_session::historical::NoteHistoricalRoot<Self, Staking>;
    type SessionHandler = <opaque::SessionKeys as OpaqueKeys>::KeyTypeIdProviders;
    type Keys = opaque::SessionKeys;
    type WeightInfo = pallet_session::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    // TODO generate these values
    pub const BagThresholds: &'static [u64] = &[1, 100, 1_000, 1_000_000, 1_000_000_000];
}
impl pallet_bags_list::Config<pallet_bags_list::Instance1> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type ScoreProvider = Staking;
    type BagThresholds = BagThresholds;
    type Score = u64;
}

impl pallet_session::historical::Config for Runtime {
    type FullIdentification = pallet_staking::Exposure<AccountId, Balance>;
    type FullIdentificationOf = pallet_staking::ExposureOf<Runtime>;
}

impl pallet_permissions::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_permissions::weights::SubstrateWeight<Runtime>;
}

impl pallet_tables::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_tables::weights::SubstrateWeight<Runtime>;
}

impl pallet_commitments::Config for Runtime {}

impl pallet_indexing::Config<native_api::Api> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_indexing::weights::SubstrateWeight<Runtime>;
}

impl pallet_attestation::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_attestation::weights::SubstrateWeight<Runtime>;
}

impl pallet_keystore::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_keystore::weights::SubstrateWeight<Runtime>;
}

impl pallet_system_tables::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
}

impl pallet_system_contracts::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
}

impl pallet_smartcontracts::Config<native_api::Api> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_smartcontracts::weights::SubstrateWeight<Runtime>;
}

impl pallet_rewards::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    // Payout up to 3 pages per block
    type MaxPayoutsPerBlock = ConstU32<3>;
}

// Create the runtime by composing the FRAME pallets that were previously configured.
#[frame_support::runtime]
mod runtime {
    #[runtime::runtime]
    #[runtime::derive(
        RuntimeCall,
        RuntimeEvent,
        RuntimeError,
        RuntimeOrigin,
        RuntimeFreezeReason,
        RuntimeHoldReason,
        RuntimeSlashReason,
        RuntimeLockId,
        RuntimeTask
    )]
    pub struct Runtime;

    #[runtime::pallet_index(0)]
    pub type System = frame_system::Pallet<Runtime>;

    #[runtime::pallet_index(1)]
    pub type Utility = pallet_utility::Pallet<Runtime>;

    #[runtime::pallet_index(2)]
    pub type Babe = pallet_babe::Pallet<Runtime>;

    #[runtime::pallet_index(3)]
    pub type Timestamp = pallet_timestamp::Pallet<Runtime>;

    // Authorship must be before session in order to note author in the correct session and era
    // for im-online and staking.
    #[runtime::pallet_index(4)]
    pub type Authorship = pallet_authorship::Pallet<Runtime>;

    #[runtime::pallet_index(6)]
    pub type Balances = pallet_balances::Pallet<Runtime>;

    #[runtime::pallet_index(7)]
    pub type TransactionPayment = pallet_transaction_payment::Pallet<Runtime>;

    #[runtime::pallet_index(10)]
    pub type ElectionProviderMultiPhase = pallet_election_provider_multi_phase::Pallet<Runtime>;

    #[runtime::pallet_index(11)]
    pub type Staking = pallet_staking::Pallet<Runtime>;

    #[runtime::pallet_index(12)]
    pub type Session = pallet_session::Pallet<Runtime>;

    #[runtime::pallet_index(18)]
    pub type Grandpa = pallet_grandpa::Pallet<Runtime>;

    #[runtime::pallet_index(22)]
    pub type Sudo = pallet_sudo::Pallet<Runtime>;

    #[runtime::pallet_index(23)]
    pub type ImOnline = pallet_im_online::Pallet<Runtime>;

    #[runtime::pallet_index(24)]
    pub type AuthorityDiscovery = pallet_authority_discovery::Pallet<Runtime>;

    #[runtime::pallet_index(25)]
    pub type Offences = pallet_offences::Pallet<Runtime>;

    #[runtime::pallet_index(26)]
    pub type Historical = pallet_session::historical::Pallet<Runtime>;

    #[runtime::pallet_index(27)]
    pub type Multisig = pallet_multisig::Pallet<Runtime>;

    #[runtime::pallet_index(52)]
    pub type VoterList = pallet_bags_list::Pallet<Runtime, Instance1>;

    #[runtime::pallet_index(71)]
    pub type Statement = pallet_statement;

    #[runtime::pallet_index(72)]
    pub type MultiBlockMigrations = pallet_migrations;

    // Custom pallets start at index 100 to ensure room for future consensus work
    #[runtime::pallet_index(100)]
    pub type Permissions = pallet_permissions::Pallet<Runtime>;
    #[runtime::pallet_index(101)]
    pub type Tables = pallet_tables::Pallet<Runtime>;
    #[runtime::pallet_index(102)]
    pub type Indexing = pallet_indexing::native_pallet::Pallet<Runtime>;
    #[runtime::pallet_index(103)]
    pub type Commitments = pallet_commitments::Pallet<Runtime>;
    #[runtime::pallet_index(104)]
    pub type Attestations = pallet_attestation::Pallet<Runtime>;
    #[runtime::pallet_index(105)]
    pub type Keystore = pallet_keystore;
    #[runtime::pallet_index(106)]
    pub type Smartcontracts = pallet_smartcontracts::native_pallet::Pallet<Runtime>;
    #[runtime::pallet_index(107)]
    pub type SystemTables = pallet_system_tables;
    #[runtime::pallet_index(108)]
    pub type SystemContracts = pallet_system_contracts;
    #[runtime::pallet_index(109)]
    pub type Rewards = pallet_rewards;
}

/// The address format for describing accounts.
pub type Address = sp_runtime::MultiAddress<AccountId, ()>;
/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
    frame_system::CheckNonZeroSender<Runtime>,
    frame_system::CheckSpecVersion<Runtime>,
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckEra<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
);

/// All migrations of the runtime, aside from the ones declared in the pallets.
///
/// This can be a tuple of types, each implementing `OnRuntimeUpgrade`.
#[allow(unused_parens)]
type Migrations = ();

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
    generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;
/// The payload being signed in transactions.
pub type SignedPayload = generic::SignedPayload<RuntimeCall, SignedExtra>;
/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
    Runtime,
    Block,
    frame_system::ChainContext<Runtime>,
    Runtime,
    AllPalletsWithSystem,
    Migrations,
>;

#[cfg(feature = "runtime-benchmarks")]
mod benches {
    frame_benchmarking::define_benchmarks!(
        [frame_benchmarking, BaselineBench::<Runtime>]
        [pallet_babe, Babe]
        [pallet_bags_list, VoterList]
        [pallet_balances, Balances]
        [pallet_election_provider_multi_phase, ElectionProviderMultiPhase]
        [pallet_grandpa, Grandpa]
        [pallet_im_online, ImOnline]
        [pallet_staking, Staking]
        [pallet_sudo, Sudo]
        [pallet_multisig, Multisig]
        [pallet_migrations, MultiBlockMigrations]
        [frame_system, SystemBench::<Runtime>]
        [pallet_timestamp, Timestamp]
        [pallet_utility, Utility]
        [pallet_permissions, Permissions]
        [pallet_indexing, Indexing]
        [pallet_attestation, Attestations]
        [pallet_keystore, Keystore]
    );
}

impl_runtime_apis! {
    impl sp_api::Core<Block> for Runtime {
        fn version() -> RuntimeVersion {
            VERSION
        }

        fn execute_block(block: Block) {
            Executive::execute_block(block);
        }

        fn initialize_block(header: &<Block as BlockT>::Header) -> sp_runtime::ExtrinsicInclusionMode {
            Executive::initialize_block(header)
        }
    }

    impl sp_api::Metadata<Block> for Runtime {
        fn metadata() -> OpaqueMetadata {
            OpaqueMetadata::new(Runtime::metadata().into())
        }

        fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> {
            Runtime::metadata_at_version(version)
        }

        fn metadata_versions() -> Vec<u32> {
            Runtime::metadata_versions()
        }
    }

    impl sp_block_builder::BlockBuilder<Block> for Runtime {
        fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
            Executive::apply_extrinsic(extrinsic)
        }

        fn finalize_block() -> <Block as BlockT>::Header {
            Executive::finalize_block()
        }

        fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
            data.create_extrinsics()
        }

        fn check_inherents(
            block: Block,
            data: sp_inherents::InherentData,
        ) -> sp_inherents::CheckInherentsResult {
            data.check_extrinsics(&block)
        }
    }

    impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
        fn validate_transaction(
            source: TransactionSource,
            tx: <Block as BlockT>::Extrinsic,
            block_hash: <Block as BlockT>::Hash,
        ) -> TransactionValidity {
            Executive::validate_transaction(source, tx, block_hash)
        }
    }

    impl sp_statement_store::runtime_api::ValidateStatement<Block> for Runtime {
        fn validate_statement(
            source: sp_statement_store::runtime_api::StatementSource,
            statement: sp_statement_store::Statement,
        ) -> Result<sp_statement_store::runtime_api::ValidStatement, sp_statement_store::runtime_api::InvalidStatement> {
            Statement::validate_statement(source, statement)
        }
    }

    impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
        fn offchain_worker(header: &<Block as BlockT>::Header) {
            Executive::offchain_worker(header)
        }
    }

    impl sp_consensus_grandpa::GrandpaApi<Block> for Runtime {
        fn grandpa_authorities() -> sp_consensus_grandpa::AuthorityList {
            Grandpa::grandpa_authorities()
        }

        fn current_set_id() -> sp_consensus_grandpa::SetId {
            Grandpa::current_set_id()
        }

        fn submit_report_equivocation_unsigned_extrinsic(
            equivocation_proof: sp_consensus_grandpa::EquivocationProof<
                <Block as BlockT>::Hash,
                NumberFor<Block>,
            >,
            key_owner_proof: sp_consensus_grandpa::OpaqueKeyOwnershipProof,
        ) -> Option<()> {
            let key_owner_proof = key_owner_proof.decode()?;

            Grandpa::submit_unsigned_equivocation_report(
                equivocation_proof,
                key_owner_proof,
            )
        }

        fn generate_key_ownership_proof(
            _set_id: sp_consensus_grandpa::SetId,
            authority_id: GrandpaId,
        ) -> Option<sp_consensus_grandpa::OpaqueKeyOwnershipProof> {
            use codec::Encode;

            Historical::prove((sp_consensus_grandpa::KEY_TYPE, authority_id))
                .map(|p| p.encode())
                .map(sp_consensus_grandpa::OpaqueKeyOwnershipProof::new)
        }
    }

    impl pallet_staking_runtime_api::StakingApi<Block, Balance, AccountId> for Runtime {
        fn nominations_quota(balance: Balance) -> u32 {
            Staking::api_nominations_quota(balance)
        }

        fn eras_stakers_page_count(era: sp_staking::EraIndex, account: AccountId) -> sp_staking::Page {
            Staking::api_eras_stakers_page_count(era, account)
        }

        fn pending_rewards(era: sp_staking::EraIndex, account: AccountId) -> bool {
            Staking::api_pending_rewards(era, account)
        }
    }

    impl sp_consensus_babe::BabeApi<Block> for Runtime {
        fn configuration() -> sp_consensus_babe::BabeConfiguration {
            let epoch_config = Babe::epoch_config().unwrap_or(BABE_GENESIS_EPOCH_CONFIG);
            sp_consensus_babe::BabeConfiguration {
                slot_duration: Babe::slot_duration(),
                epoch_length: EpochDuration::get(),
                c: epoch_config.c,
                authorities: Babe::authorities().to_vec(),
                randomness: Babe::randomness(),
                allowed_slots: epoch_config.allowed_slots,
            }
        }

        fn current_epoch_start() -> sp_consensus_babe::Slot {
            Babe::current_epoch_start()
        }

        fn current_epoch() -> sp_consensus_babe::Epoch {
            Babe::current_epoch()
        }

        fn next_epoch() -> sp_consensus_babe::Epoch {
            Babe::next_epoch()
        }

        fn generate_key_ownership_proof(
            _slot: sp_consensus_babe::Slot,
            authority_id: sp_consensus_babe::AuthorityId,
        ) -> Option<sp_consensus_babe::OpaqueKeyOwnershipProof> {
            use codec::Encode;

            Historical::prove((sp_consensus_babe::KEY_TYPE, authority_id))
                .map(|p| p.encode())
                .map(sp_consensus_babe::OpaqueKeyOwnershipProof::new)
        }

        fn submit_report_equivocation_unsigned_extrinsic(
            equivocation_proof: sp_consensus_babe::EquivocationProof<<Block as BlockT>::Header>,
            key_owner_proof: sp_consensus_babe::OpaqueKeyOwnershipProof,
        ) -> Option<()> {
            let key_owner_proof = key_owner_proof.decode()?;

            Babe::submit_unsigned_equivocation_report(
                equivocation_proof,
                key_owner_proof,
            )
        }
    }

    impl sp_session::SessionKeys<Block> for Runtime {
        fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
            opaque::SessionKeys::generate(seed)
        }

        fn decode_session_keys(
            encoded: Vec<u8>,
        ) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
            opaque::SessionKeys::decode_into_raw_public_keys(&encoded)
        }
    }

    impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce> for Runtime {
        fn account_nonce(account: AccountId) -> Nonce {
            System::account_nonce(account)
        }
    }

    impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
        fn query_info(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32,
        ) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
            TransactionPayment::query_info(uxt, len)
        }
        fn query_fee_details(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32,
        ) -> pallet_transaction_payment::FeeDetails<Balance> {
            TransactionPayment::query_fee_details(uxt, len)
        }
        fn query_weight_to_fee(weight: Weight) -> Balance {
            TransactionPayment::weight_to_fee(weight)
        }
        fn query_length_to_fee(length: u32) -> Balance {
            TransactionPayment::length_to_fee(length)
        }
    }

    impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentCallApi<Block, Balance, RuntimeCall>
        for Runtime
    {
        fn query_call_info(
            call: RuntimeCall,
            len: u32,
        ) -> pallet_transaction_payment::RuntimeDispatchInfo<Balance> {
            TransactionPayment::query_call_info(call, len)
        }
        fn query_call_fee_details(
            call: RuntimeCall,
            len: u32,
        ) -> pallet_transaction_payment::FeeDetails<Balance> {
            TransactionPayment::query_call_fee_details(call, len)
        }
        fn query_weight_to_fee(weight: Weight) -> Balance {
            TransactionPayment::weight_to_fee(weight)
        }
        fn query_length_to_fee(length: u32) -> Balance {
            TransactionPayment::length_to_fee(length)
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    impl frame_benchmarking::Benchmark<Block> for Runtime {
        fn benchmark_metadata(extra: bool) -> (
            Vec<frame_benchmarking::BenchmarkList>,
            Vec<frame_support::traits::StorageInfo>,
        ) {
            use frame_benchmarking::{baseline, Benchmarking, BenchmarkList};
            use frame_support::traits::StorageInfoTrait;
            use frame_system_benchmarking::Pallet as SystemBench;
            use baseline::Pallet as BaselineBench;

            let mut list = Vec::<BenchmarkList>::new();
            list_benchmarks!(list, extra);

            let storage_info = AllPalletsWithSystem::storage_info();

            (list, storage_info)
        }

        fn dispatch_benchmark(
            config: frame_benchmarking::BenchmarkConfig
        ) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
            use frame_benchmarking::{baseline, Benchmarking, BenchmarkBatch};
            use sp_storage::TrackedStorageKey;
            use frame_system_benchmarking::Pallet as SystemBench;
            use baseline::Pallet as BaselineBench;

            impl frame_system_benchmarking::Config for Runtime {}
            impl baseline::Config for Runtime {}

            use frame_support::traits::WhitelistedStorageKeys;
            let whitelist: Vec<TrackedStorageKey> = AllPalletsWithSystem::whitelisted_storage_keys();

            let mut batches = Vec::<BenchmarkBatch>::new();
            let params = (&config, &whitelist);
            add_benchmarks!(params, batches);

            Ok(batches)
        }
    }

    #[cfg(feature = "try-runtime")]
    impl frame_try_runtime::TryRuntime<Block> for Runtime {
        fn on_runtime_upgrade(checks: frame_try_runtime::UpgradeCheckSelect) -> (Weight, Weight) {
            // NOTE: intentional unwrap: we don't want to propagate the error backwards, and want to
            // have a backtrace here. If any of the pre/post migration checks fail, we shall stop
            // right here and right now.
            let weight = Executive::try_runtime_upgrade(checks).unwrap();
            (weight, BlockWeights::get().max_block)
        }

        fn execute_block(
            block: Block,
            state_root_check: bool,
            signature_check: bool,
            select: frame_try_runtime::TryStateSelect
        ) -> Weight {
            // NOTE: intentional unwrap: we don't want to propagate the error backwards, and want to
            // have a backtrace here.
            Executive::try_execute_block(block, state_root_check, signature_check, select).expect("execute-block failed")
        }
    }

    impl sp_genesis_builder::GenesisBuilder<Block> for Runtime {
        fn build_state(config: Vec<u8>) -> sp_genesis_builder::Result {
            build_state::<RuntimeGenesisConfig>(config)
        }

        fn get_preset(id: &Option<sp_genesis_builder::PresetId>) -> Option<Vec<u8>> {
            get_preset::<RuntimeGenesisConfig>(id, |_| None)
        }

        fn preset_names() -> Vec<sp_genesis_builder::PresetId> {
            vec![]
        }
    }

        impl sp_authority_discovery::AuthorityDiscoveryApi<Block> for Runtime {
        fn authorities() -> Vec<AuthorityDiscoveryId> {
            AuthorityDiscovery::authorities()
        }
    }

    impl pallet_commitments::runtime_api::CommitmentsApi<Block> for Runtime {
        fn table_commitments_any_scheme(table_identifiers: pallet_commitments::runtime_api::CommitmentsApiBoundedTableIdentifiersList) -> Option<pallet_commitments::AnyTableCommitments> {
            Commitments::table_commitments_any_scheme(table_identifiers.as_slice())
        }
    }
}
