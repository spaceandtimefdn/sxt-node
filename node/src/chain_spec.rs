use std::env;

use dotenv::dotenv;
use sc_chain_spec::ChainSpecExtension;
use sc_service::{ChainType, Properties};
use serde::{Deserialize, Serialize};
use sp_authority_discovery::AuthorityId;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::{sr25519, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};
use sxt_runtime::opaque::SessionKeys;
use sxt_runtime::{
    AccountId,
    Balance,
    Block,
    Perbill,
    Signature,
    BABE_GENESIS_EPOCH_CONFIG,
    DOLLARS,
    WASM_BINARY,
};

// The URL for the telemetry server.
// const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

/// Helper struct that contains each component required to configure a validator node during the
/// genesis creation
#[derive(Clone)]
pub struct NodeIdSet {
    pub controller: AccountId,
    pub stash: AccountId,
    pub grandpa: GrandpaId,
    pub babe: BabeId,
    pub authority_discovery: AuthorityId,
}

/// This struct defines extension modules that will be needed in generating and parsing
/// the chain spec
#[derive(Default, Clone, Serialize, Deserialize, ChainSpecExtension)]
#[serde(rename_all = "camelCase")]
pub struct Extensions {
    /// Block numbers with known hashes.
    pub fork_blocks: sc_client_api::ForkBlocks<Block>,
    /// Known bad block hashes.
    pub bad_blocks: sc_client_api::BadBlocks<Block>,
    /// The light sync state extension used by the sync-state rpc.
    pub light_sync_state: sc_sync_state_rpc::LightSyncStateExtension,
}

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<Extensions>;

/// Generate a crypto pair from seed.
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
    TPublic::Pair::from_string(&format!("//{}", seed), None)
        .expect("static values are valid; qed")
        .public()
}

pub fn get_from_phrase<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
    TPublic::Pair::from_string(seed, None)
        .expect("static values are valid; qed")
        .public()
}

type AccountPublic = <Signature as Verify>::Signer;

/// Generate an account ID from seed.
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
    AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
    AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Generate an account ID from seed.
pub fn get_account_id_from_phrase<TPublic: Public>(seed: &str) -> AccountId
where
    AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
    AccountPublic::from(get_from_phrase::<TPublic>(seed)).into_account()
}

/// Helper function to generate stash, controller and session key from seed.
pub fn authority_keys_from_seed(s: &str) -> NodeIdSet {
    NodeIdSet {
        controller: get_from_seed::<sr25519::Public>(s).into(),
        stash: get_from_seed::<sr25519::Public>(s).into(),
        grandpa: get_from_seed::<GrandpaId>(s),
        babe: get_from_seed::<BabeId>(s),
        authority_discovery: get_from_seed::<AuthorityId>(s),
    }
}

pub fn authority_keys_from_phrase(s: &str) -> NodeIdSet {
    NodeIdSet {
        controller: get_account_id_from_phrase::<sr25519::Public>(s),
        stash: get_account_id_from_phrase::<sr25519::Public>(s),
        grandpa: get_from_phrase::<GrandpaId>(s),
        babe: get_from_phrase::<BabeId>(s),
        authority_discovery: get_from_seed::<AuthorityId>(s),
    }
}

pub fn devnet_config() -> Result<ChainSpec, String> {
    Ok(ChainSpec::builder(
        WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?,
        Default::default(),
    )
    .with_name("SxT Devnet")
    .with_id("devnet")
    .with_chain_type(ChainType::Live)
    .with_properties(token_properties())
    .with_genesis_config_patch(testnet_genesis(
        // Initial NPoS authorities
        vec![authority_keys_from_seed("Alice")],
        vec![get_account_id_from_seed::<sr25519::Public>("Bob")],
        // Sudo account
        get_account_id_from_seed::<sr25519::Public>("Alice"),
        // Pre-funded accounts
        vec![
            get_account_id_from_seed::<sr25519::Public>("Alice"),
            get_account_id_from_seed::<sr25519::Public>("Bob"),
            get_account_id_from_seed::<sr25519::Public>("Charlie"),
            get_account_id_from_seed::<sr25519::Public>("Dave"),
            get_account_id_from_seed::<sr25519::Public>("Eve"),
            get_account_id_from_seed::<sr25519::Public>("Ferdie"),
            get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
            get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
            get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
            get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
            get_account_id_from_seed::<sr25519::Public>("Eve//stash"),
            get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"),
        ],
        true,
    ))
    .build())
}

pub fn development_config() -> Result<ChainSpec, String> {
    Ok(ChainSpec::builder(
        WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?,
        Default::default(),
    )
    .with_name("Development")
    .with_id("dev")
    .with_chain_type(ChainType::Development)
    .with_properties(token_properties())
    .with_genesis_config_patch(testnet_genesis(
        // Initial NPoS authorities
        vec![authority_keys_from_seed("Alice")],
        vec![get_account_id_from_seed::<sr25519::Public>("Charlie")],
        // Sudo account
        get_account_id_from_seed::<sr25519::Public>("Alice"),
        // Pre-funded accounts
        vec![
            get_account_id_from_seed::<sr25519::Public>("Alice"),
            get_account_id_from_seed::<sr25519::Public>("Bob"),
            get_account_id_from_seed::<sr25519::Public>("Charlie"),
            get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
            get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
            get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
        ],
        true,
    ))
    .build())
}

pub fn local_testnet_config() -> Result<ChainSpec, String> {
    Ok(ChainSpec::builder(
        WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?,
        Default::default(),
    )
    .with_name("Sxt Testnet")
    .with_id("sxt-testnet")
    .with_chain_type(ChainType::Local)
    .with_properties(token_properties())
    .with_genesis_config_patch(testnet_genesis(
        // Initial NPoS authorities
        vec![
            authority_keys_from_seed("Alice"),
            authority_keys_from_seed("Bob"),
        ],
        vec![
            get_account_id_from_seed::<sr25519::Public>("Charlie"),
            get_account_id_from_seed::<sr25519::Public>("Dave"),
        ],
        // Sudo account
        get_account_id_from_seed::<sr25519::Public>("Alice"),
        // Pre-funded accounts
        vec![
            get_account_id_from_seed::<sr25519::Public>("Alice"),
            get_account_id_from_seed::<sr25519::Public>("Bob"),
            get_account_id_from_seed::<sr25519::Public>("Charlie"),
            get_account_id_from_seed::<sr25519::Public>("Dave"),
            get_account_id_from_seed::<sr25519::Public>("Eve"),
            get_account_id_from_seed::<sr25519::Public>("Ferdie"),
            get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
            get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
            get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
            get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
            get_account_id_from_seed::<sr25519::Public>("Eve//stash"),
            get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"),
        ],
        true,
    ))
    .build())
}

fn get_env_or_panic(input: &str) -> String {
    env::var(input).unwrap_or_else(|_| panic!("ERROR: {} ENV variable not set", input))
}

fn validators_or_panic() -> (String, String, String) {
    (
        get_env_or_panic("SXT_VALIDATOR_1"),
        get_env_or_panic("SXT_VALIDATOR_2"),
        get_env_or_panic("SXT_VALIDATOR_3"),
    )
}

fn indexers_or_panic() -> (String, String, String, String, String) {
    (
        get_env_or_panic("SXT_INDEXER_1"),
        get_env_or_panic("SXT_INDEXER_2"),
        get_env_or_panic("SXT_INDEXER_3"),
        get_env_or_panic("SXT_INDEXER_4"),
        get_env_or_panic("SXT_INDEXER_5"),
    )
}

fn sudo_key_or_panic() -> String {
    get_env_or_panic("SXT_SUDO_KEY")
}

pub fn production_config() -> Result<ChainSpec, String> {
    dotenv().ok();

    let (validator1, validator2, validator3) = validators_or_panic();
    let (indexer1, indexer2, indexer3, indexer4, indexer5) = indexers_or_panic();
    let sudo_key = sudo_key_or_panic();

    Ok(ChainSpec::builder(
        WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?,
        Default::default(),
    )
    .with_name("Sxt Testnet")
    .with_id("sxt-testnet")
    .with_chain_type(ChainType::Live)
    .with_properties(token_properties())
    .with_genesis_config_patch(testnet_genesis(
        // Initial NPoS authorities
        vec![
            // Initial Validators
            authority_keys_from_phrase(&validator1),
            authority_keys_from_phrase(&validator2),
            authority_keys_from_phrase(&validator3),
        ],
        vec![
            // Initial Nominators
            get_account_id_from_phrase::<sr25519::Public>(&indexer1),
            get_account_id_from_phrase::<sr25519::Public>(&indexer2),
            get_account_id_from_phrase::<sr25519::Public>(&indexer3),
            get_account_id_from_phrase::<sr25519::Public>(&indexer4),
            get_account_id_from_phrase::<sr25519::Public>(&indexer5),
        ],
        // Sudo account
        get_account_id_from_phrase::<sr25519::Public>(&sudo_key),
        // Pre-funded accounts
        vec![
            get_account_id_from_phrase::<sr25519::Public>(&sudo_key),
            get_account_id_from_phrase::<sr25519::Public>(&validator1),
            get_account_id_from_phrase::<sr25519::Public>(&validator2),
            get_account_id_from_phrase::<sr25519::Public>(&validator3),
            get_account_id_from_phrase::<sr25519::Public>(&indexer1),
            get_account_id_from_phrase::<sr25519::Public>(&indexer2),
            get_account_id_from_phrase::<sr25519::Public>(&indexer3),
            get_account_id_from_phrase::<sr25519::Public>(&indexer4),
            get_account_id_from_phrase::<sr25519::Public>(&indexer5),
        ],
        true,
    ))
    .build())
}

/// Returns the token properties as a ha
fn token_properties() -> Properties {
    let mut map = serde_json::Map::new();

    map.insert(
        "tokenSymbol".into(),
        serde_json::Value::String("SxT".into()),
    );
    map.insert("tokenDecimals".into(), serde_json::Value::Number(18.into()));

    map
}

#[allow(clippy::type_complexity)]
fn configure_accounts(
    initial_authorities: Vec<NodeIdSet>,
    initial_nominators: Vec<AccountId>,
    mut endowed_accounts: Vec<AccountId>,
    stash: Balance,
) -> (
    Vec<NodeIdSet>,
    Vec<AccountId>,
    Vec<(
        AccountId,
        AccountId,
        Balance,
        pallet_staking::StakerStatus<AccountId>,
    )>,
) {
    // endow all authorities and nominators.
    initial_authorities
        .iter()
        .map(|x| &x.controller)
        .chain(initial_nominators.iter())
        .for_each(|x| {
            if !endowed_accounts.contains(x) {
                endowed_accounts.push(x.clone())
            }
        });

    // stakers: all validators and nominators.
    let stakers = initial_authorities
        .iter()
        .map(|x| {
            (
                x.controller.clone(),
                x.stash.clone(),
                stash,
                pallet_staking::StakerStatus::Validator,
            )
        })
        .chain(initial_nominators.iter().map(|x| {
            // Add all authorities to all nominators
            let nominations = initial_authorities
                .clone()
                .into_iter()
                .map(|target| target.controller.clone())
                .collect::<Vec<_>>();
            (
                x.clone(),
                x.clone(),
                stash,
                pallet_staking::StakerStatus::Nominator(nominations),
            )
        }))
        .collect::<Vec<_>>();

    (initial_authorities, endowed_accounts, stakers)
}

/// Configure initial storage state for FRAME modules.
fn testnet_genesis(
    initial_authorities: Vec<NodeIdSet>,
    initial_nominators: Vec<AccountId>,
    root_key: AccountId,
    endowed_accounts: Vec<AccountId>,
    _enable_println: bool,
) -> serde_json::Value {
    const ENDOWMENT: Balance = 10_000_000 * DOLLARS;
    const STASH: Balance = ENDOWMENT / 1000;

    let (initial_authorities, endowed_accounts, stakers) = configure_accounts(
        initial_authorities,
        initial_nominators,
        endowed_accounts,
        STASH,
    );

    serde_json::json!({
        "balances": {
            "balances": endowed_accounts.iter().cloned().map(|k| (k, ENDOWMENT)).collect::<Vec<_>>(),
        },
        "session": {
            "keys": initial_authorities.iter().map(|x| {
                (x.controller.clone(), x.stash.clone(), SessionKeys { grandpa: x.grandpa.clone(), babe: x.babe.clone(), authority_discovery: x.authority_discovery.clone() })
            }).collect::<Vec<_>>(),
        },
        "staking": {
            "validatorCount": initial_authorities.len() as u32,
            "minimumValidatorCount": initial_authorities.len() as u32,
            "maxNominatorCount": 22_500u32,
            "maxValidatorCount": 10u32,
            "invulnerables": initial_authorities.iter().map(|x| x.controller.clone()).collect::<Vec<_>>(),
            "slashRewardFraction": Perbill::from_percent(10),
            "stakers": stakers,
        },
        "sudo": {
            // Assign network admin rights.
            "key": Some(root_key.clone()),
        },
        "babe": {
            "epochConfig": Some(BABE_GENESIS_EPOCH_CONFIG),
        },
    })
}

fn session_keys(grandpa: GrandpaId, babe: BabeId, authority_discovery: AuthorityId) -> SessionKeys {
    SessionKeys {
        babe,
        grandpa,
        authority_discovery,
    }
}
