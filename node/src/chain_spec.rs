use std::env;
use std::fs::read_to_string;

use dotenv::dotenv;
use jsonrpsee::tracing::log;
use proof_of_sql_commitment_map::TableCommitmentBytesPerCommitmentScheme;
use sc_chain_spec::ChainSpecExtension;
use sc_service::{ChainType, Properties};
use serde::{Deserialize, Serialize};
use sp_authority_discovery::AuthorityId;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::{sr25519, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};
use sqlparser::ast::helpers::stmt_create_table::CreateTableBuilder;
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;
use sxt_core::tables::{
    create_statement,
    table_identifier,
    ColumnUuidList,
    IndexerMode,
    InsertQuorumSize,
    RawGenesisTable,
    SnapshotUrl,
    Source,
    SourceAndMode,
    TableIdentifier,
    TableUuid,
    TableVersion,
};
use sxt_core::ByteString;
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

    let default_quorum_size = InsertQuorumSize {
        public: Some(3),
        privileged: Some(0),
    };

    let test = serde_json::json!({
    "test": ddls_to_genesis(vec![
                    (
                        "snapshots/v2/ethereum_core/ddl_ethereum_snapshot_v2.sql".into(),
                        ethereum_core(),
                        "snapshots/v2/ethereum_core/url_snapshot_v2.url".into(),
                        default_quorum_size,
                        0,
                        Default::default(),
                        Default::default(),
                    )
                ]),
    });

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
        "tables": {
            // "tables": pair_commits( ddls_to_genesis(vec![]), vec![] ),
            "tablesWithoutCommits": ddls_to_genesis(vec![
                    // DdlPath,
                    //         SourceAndMode,
                    //         SnapshotPath,
                    //         InsertQuorumSize,
                    //         TableVersion,
                    //         TableUuid,
                    //         ColumnUuidList,
                    (
                        "snapshots/v3/sxt_system_staking/ddl_sxt_system_staking.sql".into(),
                        sepolia_staking(),
                        "snapshots/v3/sxt_system_staking/url_snapshot_v3.url".into(),
                        default_quorum_size,
                        0,
                        Default::default(),
                        Default::default(),
                    ),
                (
                        "snapshots/v2/ethereum_core/ddl_ethereum_snapshot_v2.sql".into(),
                        sepolia_staking(),
                        "snapshots/v2/ethereum_core/url_snapshot_v2.url".into(),
                        default_quorum_size,
                        0,
                        Default::default(),
                        Default::default(),
                    ),
                // (
                //         "snapshots/v3/ethereum_beacon/ddl_ethereum_beacon_snapshot_v3.sql".into(),
                //         sepolia_staking(),
                //         "snapshots/v3/ethereum_beacon/url_snapshot_v3.url".into(),
                //         default_quorum_size,
                //         0,
                //         Default::default(),
                //         Default::default(),
                //     ),
            ])
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

/// Ethereum Core source and mode
pub fn ethereum_core() -> SourceAndMode {
    SourceAndMode {
        source: Source::Ethereum,
        mode: IndexerMode::Core,
    }
}

const SEPOLIA_STAKING_CONTRACT: &str = "0x99b712919F0c2C07ad32f4c3a3742D3C6642d0A2";
pub fn sepolia_staking() -> SourceAndMode {
    let contract_byte_string =
        ByteString::try_from(SEPOLIA_STAKING_CONTRACT.as_bytes().to_vec()).unwrap();
    SourceAndMode {
        source: Source::Sepolia,
        mode: IndexerMode::SmartContract(contract_byte_string),
    }
}

/// Create table list
pub type CreateTableList = Vec<(RawGenesisTable, TableCommitmentBytesPerCommitmentScheme)>;

/// List of ddsl
pub type DdlList = Vec<RawGenesisTable>;

/// List of commitments
pub type CommitmentList = Vec<(TableIdentifier, TableCommitmentBytesPerCommitmentScheme)>;

/// Pair a ddl list with a list of commits based on table identifier to form a create table list
pub fn pair_commits(input: DdlList, paths: Vec<String>) -> CreateTableList {
    let mut output: CreateTableList = Vec::new();
    let mut commits: CommitmentList = Vec::new();

    for p in paths.iter() {
        let json_str =
            std::fs::read_to_string(p).unwrap_or_else(|_| panic!("Could not read path {}", p));
        let commits_for_file: CommitmentList =
            serde_json::from_str(&json_str).expect("could not parse commitments");

        commits.extend(commits_for_file);
    }

    if commits.len() != input.len() {
        panic!(
            "Found {} tables but only {} commits",
            input.len(),
            commits.len()
        );
    }

    for table in input.into_iter() {
        for i in 0..commits.len() {
            let commit = commits.get(i).expect("could not get commit");

            if table.table_identifier == commit.0 {
                let entry: RawGenesisTable = RawGenesisTable {
                    source_and_mode: table.source_and_mode,
                    table_identifier: table.table_identifier,
                    create_statement: table.create_statement,
                    snapshot_url: table.snapshot_url,
                    insert_quorum_size: table.insert_quorum_size,
                    table_version: table.table_version,
                    table_uuid: table.table_uuid,
                    namespace_uuid: Default::default(),
                    column_uuid_list: table.column_uuid_list,
                };
                output.push((entry, commit.1.clone()));
                commits.remove(i);
                break;
            }
        }
    }

    if !commits.is_empty() {
        log::warn!("not all commits were utilized")
    }

    output
}

/// Read a ddl file from a path and parse it into create table statements, any error will cause a panic
pub fn ddl_to_tables(
    p: String,
    sm: SourceAndMode,
    snapshot: String,
    quorum: InsertQuorumSize,
    version: TableVersion,
    uuid: TableUuid,
    column_uuids: ColumnUuidList,
) -> Vec<RawGenesisTable> {
    let ddl = read_to_string(p).unwrap();

    let snapshot_url = read_to_string(snapshot).unwrap();
    let snapshot_url = SnapshotUrl::try_from(snapshot_url.as_bytes().to_vec()).unwrap();

    let mut parser = Parser::new(&PostgreSqlDialect {})
        .try_with_sql(ddl.as_str())
        .unwrap();
    let statements = parser.parse_statements().unwrap();

    #[allow(clippy::unnecessary_filter_map)]
    statements
        .into_iter()
        .filter_map(|x| match CreateTableBuilder::try_from(x.clone()) {
            Ok(c) => {
                let name = c.name.to_string();
                let pieces: Vec<&str> = name.split(".").collect();
                let namespace = pieces.first().unwrap();
                let name = pieces.get(1).unwrap();
                let s = c.build().to_string();
                let sm = sm.clone();

                Some(RawGenesisTable {
                    source_and_mode: sm,
                    table_identifier: table_identifier(name, namespace),
                    create_statement: create_statement(&s),
                    snapshot_url: snapshot_url.clone(),
                    insert_quorum_size: quorum,
                    table_version: version,
                    table_uuid: uuid.clone(),
                    namespace_uuid: Default::default(),
                    column_uuid_list: column_uuids.clone(),
                })
            }
            Err(_) => panic!("Error parsing table {}", x),
        })
        .collect()
}

/// A path to DDL files represented by a string
pub type DdlPath = String;

/// A path to a url file containing the snapshot
pub type SnapshotPath = String;

/// Convert a vector of ddl paths and source and modes into a vector of tables for genesis configuration
pub fn ddls_to_genesis(
    input: Vec<(
        DdlPath,
        SourceAndMode,
        SnapshotPath,
        InsertQuorumSize,
        TableVersion,
        TableUuid,
        ColumnUuidList,
    )>,
) -> Vec<RawGenesisTable> {
    input
        .iter()
        .flat_map(
            |(path, sm, snapshot, quorum_size, version, uuid, column_uuids)| {
                ddl_to_tables(
                    path.clone(),
                    sm.clone(),
                    snapshot.clone(),
                    *quorum_size,
                    *version,
                    uuid.clone(),
                    column_uuids.clone(),
                )
            },
        )
        .collect()
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn pair_commits_works() {
        let input = ddls_to_genesis(vec![(
            "testing/test_ddl.sql".into(),
            ethereum_core(),
            "testing/test.url".into(),
            InsertQuorumSize {
                public: Some(1),
                privileged: Some(1),
            },
            0,
            Default::default(),
            Default::default(),
        )]);

        let paths = vec!["testing/test.commits".into()];

        pair_commits(input, paths);
    }

    #[test]
    fn parse_tables_from_ddl_works() {
        ddl_to_tables(
            "testing/ddl.sql".into(),
            SourceAndMode::default(),
            "testing/test.url".into(),
            InsertQuorumSize {
                public: Some(1),
                privileged: Some(1),
            },
            0,
            Default::default(),
            Default::default(),
        );
    }

    #[test]
    fn ddls_to_genesis_works() {
        ddls_to_genesis(vec![
            (
                "testing/ddl.sql".into(),
                SourceAndMode::default(),
                "testing/test.url".into(),
                InsertQuorumSize {
                    public: Some(1),
                    privileged: Some(1),
                },
                0,
                Default::default(),
                Default::default(),
            ),
            (
                "testing/ddl2.sql".into(),
                SourceAndMode::default(),
                "testing/test.url".into(),
                InsertQuorumSize {
                    public: Some(1),
                    privileged: Some(1),
                },
                0,
                Default::default(),
                Default::default(),
            ),
        ]);
    }
}
