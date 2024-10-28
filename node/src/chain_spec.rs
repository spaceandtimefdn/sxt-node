use std::env;
use std::fs::read_to_string;

use dotenv::dotenv;
use proof_of_sql_commitment_map::TableCommitmentBytesPerCommitmentScheme;
use sc_service::{ChainType, Properties};
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::{sr25519, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};
use sp_runtime::AccountId32;
use sqlparser::ast::helpers::stmt_create_table::CreateTableBuilder;
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;
use sxt_core::tables::{
    create_statement,
    table_identifier,
    CreateStatement,
    IndexerMode,
    SnapshotUrl,
    Source,
    SourceAndMode,
    TableIdentifier,
};
use sxt_runtime::opaque::SessionKeys;
use sxt_runtime::{AccountId, Signature, WASM_BINARY};

// The URL for the telemetry server.
// const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec;

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

/// Generate an Aura authority key.
pub fn authority_keys_from_seed(s: &str) -> (AccountId, AuraId, GrandpaId) {
    (
        get_account_id_from_seed::<sr25519::Public>(s),
        get_from_seed::<AuraId>(s),
        get_from_seed::<GrandpaId>(s),
    )
}

pub fn authority_keys_from_phrase(s: &str) -> (AuraId, GrandpaId) {
    (
        get_from_phrase::<AuraId>(s),
        get_from_phrase::<GrandpaId>(s),
    )
}

pub fn development_config() -> Result<ChainSpec, String> {
    Ok(ChainSpec::builder(
        WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?,
        None,
    )
    .with_name("Development")
    .with_id("dev")
    .with_chain_type(ChainType::Development)
    .with_properties(token_properties())
    .with_genesis_config_patch(testnet_genesis(
        // Initial PoA authorities
        vec![authority_keys_from_seed("Alice")],
        // Sudo account
        get_account_id_from_seed::<sr25519::Public>("Alice"),
        // Pre-funded accounts
        vec![
            get_account_id_from_seed::<sr25519::Public>("Alice"),
            get_account_id_from_seed::<sr25519::Public>("Bob"),
            get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
            get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
        ],
        true,
    ))
    .build())
}

pub fn local_testnet_config() -> Result<ChainSpec, String> {
    Ok(ChainSpec::builder(
        WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?,
        None,
    )
    .with_name("Sxt Testnet")
    .with_id("sxt-testnet")
    .with_chain_type(ChainType::Local)
    .with_properties(token_properties())
    .with_genesis_config_patch(testnet_genesis(
        // Initial PoA authorities
        vec![
            authority_keys_from_seed("Alice"),
            authority_keys_from_seed("Bob"),
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
        None,
    )
    .with_name("Sxt Testnet")
    .with_id("sxt-testnet")
    .with_chain_type(ChainType::Local)
    .with_properties(token_properties())
    .with_genesis_config_patch(testnet_genesis(
        // Initial PoA authorities
        vec![
            authority_keys_from_phrase(&validator1),
            authority_keys_from_phrase(&validator2),
            authority_keys_from_phrase(&validator3),
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
        serde_json::Value::String("USD-C".into()),
    );

    map
}
/// Configure initial storage state for FRAME modules.
fn testnet_genesis(
    initial_authorities: Vec<(AccountId, AuraId, GrandpaId)>,
    root_key: AccountId,
    endowed_accounts: Vec<AccountId>,
    _enable_println: bool,
) -> serde_json::Value {
    serde_json::json!({
        "balances": {
            // Configure endowed accounts with initial balance of 1 << 60.
            "balances": endowed_accounts.iter().cloned().map(|k| (k, 1u64 << 60)).collect::<Vec<_>>(),
        },
        "aura": {
            "authorities": initial_authorities.iter().map(|x| (x.1.clone())).collect::<Vec<_>>(),
        },
        "grandpa": {
            "authorities": initial_authorities.iter().map(|x| (x.2.clone(), 1)).collect::<Vec<_>>(),
        },
        "sudo": {
            // Assign network admin rights.
            "key": Some(root_key),
        },
        "session": {
            "keys": initial_authorities.iter().map(|x| {
                (x.0.clone(), x.0.clone(), SessionKeys { grandpa: x.2.clone(), aura: x.1.clone()})
            }).collect::<Vec<_>>(),
        },
        "validators": {
            "initial_validators": initial_authorities.iter().map(|x| x.0.clone()).collect::<Vec<_>>(),
        },

        "tables": {
            "tables":
                pair_commits(
                    ddls_to_genesis(vec![(
                        "snapshots/v2/ethereum_core/ddl_ethereum_snapshot_v2.sql".into(),
                        ethereum_core(),
                        "snapshots/v2/ethereum_core/url_snapshot_v2.url".into(),
                    )]),
                    vec!["snapshots/v2/ethereum_core/commitments_snapshot_v2.commits".into()],
            ),
        },
    })
}

fn session_keys(aura: AuraId, grandpa: GrandpaId) -> SessionKeys {
    SessionKeys { aura, grandpa }
}

/// Ethereum Core source and mode
pub fn ethereum_core() -> SourceAndMode {
    SourceAndMode {
        source: Source::Ethereum,
        mode: IndexerMode::Core,
    }
}

/// Create table list
pub type CreateTableList = Vec<(
    SourceAndMode,
    TableIdentifier,
    CreateStatement,
    TableCommitmentBytesPerCommitmentScheme,
    SnapshotUrl,
)>;

/// List of ddsl
pub type DdlList = Vec<(SourceAndMode, TableIdentifier, CreateStatement, SnapshotUrl)>;

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

    for (source, ident, stmnt, snapshot) in input.into_iter() {
        for i in 0..commits.len() {
            let commit = commits.get(i).expect("could not get commit");

            if ident == commit.0 {
                output.push((source, ident, stmnt, commit.1.clone(), snapshot));
                commits.remove(i);
                break;
            }
        }
    }

    if !commits.is_empty() {
        panic!("not all commits were utilized")
    }

    output
}

/// Read a ddl file from a path and parse it into create table statements, any error will cause a panic
pub fn ddl_to_tables(
    p: String,
    sm: SourceAndMode,
    snapshot: String,
) -> Vec<(SourceAndMode, TableIdentifier, CreateStatement, SnapshotUrl)> {
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

                Some((
                    sm,
                    table_identifier(name, namespace),
                    create_statement(&s),
                    snapshot_url.clone(),
                ))
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
    input: Vec<(DdlPath, SourceAndMode, SnapshotPath)>,
) -> Vec<(SourceAndMode, TableIdentifier, CreateStatement, SnapshotUrl)> {
    input
        .iter()
        .flat_map(|(path, sm, snapshot)| ddl_to_tables(path.clone(), sm.clone(), snapshot.clone()))
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
        );
    }

    #[test]
    fn ddls_to_genesis_works() {
        ddls_to_genesis(vec![
            (
                "testing/ddl.sql".into(),
                SourceAndMode::default(),
                "testing/test.url".into(),
            ),
            (
                "testing/ddl2.sql".into(),
                SourceAndMode::default(),
                "testing/test.url".into(),
            ),
        ]);
    }
}
