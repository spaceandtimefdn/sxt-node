use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::anyhow;
use log::{error, info};
use sqlparser::ast::Statement;
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser as SqlParser;
use subxt::config::polkadot::PolkadotExtrinsicParamsBuilder as Params;
use subxt::ext::subxt_core::tx::payload::StaticPayload;
use subxt::{OnlineClient, PolkadotConfig};
use subxt_signer::sr25519::Keypair;
use subxt_signer::SecretUri;
use sxt_core::sxt_chain_runtime::api::runtime_types::bounded_collections::bounded_vec::BoundedVec;
use sxt_core::sxt_chain_runtime::api::runtime_types::sxt_core::tables::TableIdentifier;
use sxt_core::sxt_chain_runtime::api::tables::calls::types::UpdateTableUuid;
use sxt_core::sxt_chain_runtime::api::tx;
use sxt_core::tables::convert_ignite_create_statement;
use tokio::sync::Mutex;
use url::Url;

use crate::common;

fn read_file(filename: &str) -> Result<String, std::io::Error> {
    info!("Reading file: {}", filename);
    fs::read_to_string(filename)
}

fn parse_sql(sql: &str) -> Result<Vec<sqlparser::ast::Statement>, sqlparser::parser::ParserError> {
    info!("Parsing SQL content");
    let dialect = GenericDialect;
    SqlParser::parse_sql(&dialect, sql)
}

fn format_statements(statements: &[sqlparser::ast::Statement]) -> Vec<String> {
    statements.iter().map(|stmt| stmt.to_string()).collect()
}

/// A helper function that takes in a parsed statement and evaluates it for a table UUID, returning
/// one if found.
/// Currently only CREATE TABLE is supported by this function.
fn extract_table_data(
    statement: &Statement,
    version: u16,
) -> Option<StaticPayload<UpdateTableUuid>> {
    if let Statement::CreateTable {
        name, with_options, ..
    } = statement
    {
        info!("Extracting table data from statement: {}", statement);
        let table_id = TableIdentifier {
            name: BoundedVec(name.0.get(1)?.value.as_bytes().to_vec()),
            namespace: BoundedVec(name.0.first()?.value.as_bytes().to_vec()),
        };

        let table_uuid: Vec<String> = with_options
            .iter()
            .filter_map(|option| {
                if option.name.value.to_uppercase() == "TABLE_UUID" {
                    Some(option.value.to_string())
                } else {
                    None
                }
            })
            .collect();

        if !table_uuid.is_empty() {
            let item = tx().tables().update_table_uuid(
                table_id,
                version,
                BoundedVec(table_uuid.first().unwrap().as_bytes().to_vec()),
            );
            return Some(item);
        }
    }
    None
}

/// Send to substrate
async fn send_to_substrate(
    statements: Vec<Statement>,
    client: Arc<Mutex<OnlineClient<PolkadotConfig>>>,
    keypair: &Keypair,
    nonce: Arc<AtomicU64>,
    table_version: u16,
) {
    let table_updates: Vec<_> = statements
        .iter()
        .filter_map(|s| extract_table_data(s, table_version))
        .collect();

    for tx in table_updates {
        let client = client.lock().await;
        let nonce_value = nonce.load(Ordering::Acquire);
        info!("Submitting transaction with nonce: {}", nonce_value);
        let tx_params = Params::new().nonce(nonce_value).build();
        nonce.fetch_add(1u64, Ordering::SeqCst);

        match client
            .tx()
            .sign_and_submit_then_watch(&tx, keypair, tx_params)
            .await
        {
            Ok(progress) => match progress.wait_for_finalized_success().await {
                Ok(_) => info!("Transaction finalized successfully"),
                Err(e) => error!("Table creation failed: {}", e),
            },
            Err(e) => error!("Failed to sign and send transaction: {}", e),
        }
    }
}

/// Handles the `update_uuids` command
pub(crate) async fn update_uuids(
    file: PathBuf,
    private_key: &str,
    rpc: &Url,
    version: u16,
) -> anyhow::Result<()> {
    let signer = Keypair::from_uri(&SecretUri::from_str(private_key)?)?;
    let ddl_content = fs::read_to_string(file)?;
    let fixed_ddl = convert_ignite_create_statement(&ddl_content);

    let statements =
        SqlParser::parse_sql(&GenericDialect, &fixed_ddl).map_err(|e| anyhow!(e.to_string()))?;
    let subxt_client = common::create_subxt_client(rpc).await?;

    let id = signer.public_key().to_account_id();
    let nonce_start = common::get_starting_nonce(subxt_client.clone(), id).await;
    let nonce = Arc::new(AtomicU64::new(nonce_start));

    send_to_substrate(statements, subxt_client, &signer, nonce, version).await;
    Ok(())
}
