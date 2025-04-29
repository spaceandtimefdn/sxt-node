use core::str::from_utf8;
use std::hash::Hash;
use std::sync::Arc;
use std::time::Duration;
use std::{env, fmt};

use arrow::error::ArrowError;
use arrow::record_batch::RecordBatch;
use arrow_flight::flight_service_client::FlightServiceClient;
use arrow_flight::sql::client::FlightSqlServiceClient;
use arrow_flight::sql::CommandStatementIngest;
use codec::Decode;
use frame_support::__private::log;
use on_chain_table::OnChainTable;
use sc_client_api::{Backend, BlockchainEvents, Finalizer, StorageKey, StorageProvider};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_core::traits::SpawnEssentialNamed;
use sp_runtime::traits::Header;
use sp_runtime_interface::sp_wasm_interface::anyhow;
use subxt::backend::rpc::reconnecting_rpc_client::RpcClient;
use subxt::blocks::Block;
use subxt::client::OfflineClientT;
use subxt::ext::futures;
use subxt::ext::futures::StreamExt;
use subxt::{OnlineClient, PolkadotConfig};
use tokio::sync::Mutex;
use tokio::time::sleep;
use tonic::transport::Channel;
#[cfg(not(doctest))] // Skip doc tests on generated file
use {
    crate::sxt_chain_runtime::api::indexing::events::QuorumReached,
    crate::sxt_chain_runtime::api::runtime_types::bounded_collections::bounded_vec::BoundedVec,
    crate::sxt_chain_runtime::api::tables::events::SchemaUpdated,
    crate::sxt_chain_runtime::api::tables::events::TablesCreatedWithCommitments,
};

/// Maximum delay between backoff retries (3 minutes)
pub const MAX_DELAY_SECONDS: u64 = 60 * 3;
/// Minimum delay between backoff retries (5 Second)
pub const MIN_DELAY_SECONDS: u64 = 5;
/// Maximum number of retries
pub const MAX_RETRY_ATTEMPTS: u32 = 5;

/// Errors relating to the sql interactions with FlightSQL
#[derive(Debug)]
pub enum SQLError {
    /// FlightSQL had an error connecting to the Database
    DBServiceError(String),
    /// We had an error connecting to the FlightSQL server
    FlightSQLServiceError(String),
    /// The table identifier was corrupt or not in UTF-8 Format
    BadTableIdentifier(String),
    /// The SQL statement was corrup or not in UTF-8 format
    BadSQLStatement(String),
    /// There was an error executing the provided SQL statement
    SQLExecutionError(String),
    /// There was an error inserting a record batch
    InsertExecutionError(String),
    /// The RecordBatch provided was either corrupt or in the incorrect format
    BadRecordBatch(String),
}

impl fmt::Display for SQLError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SQLError::DBServiceError(err) => write!(f, "Database service error: {}", err),
            SQLError::FlightSQLServiceError(err) => write!(f, "FlightSQL service error: {}", err),
            SQLError::BadTableIdentifier(err) => write!(f, "Invalid table identifier: {}", err),
            SQLError::BadSQLStatement(err) => write!(f, "Invalid SQL statement: {}", err),
            SQLError::SQLExecutionError(err) => write!(f, "SQL execution error: {}", err),
            SQLError::InsertExecutionError(err) => write!(f, "Insert execution error: {}", err),
            SQLError::BadRecordBatch(err) => write!(f, "Invalid or corrupt record batch: {}", err),
        }
    }
}

impl std::error::Error for SQLError {}

/// Wrapper to spawn the flightsql task using the provided Spawn handle.
pub fn spawn_flightsql_tasks<Client, Block, BE>(
    name: &'static str,
    spawner: &impl SpawnEssentialNamed,
    client: Arc<Client>,
) where
    Client: BlockchainEvents<Block>
        + HeaderBackend<Block>
        + ProvideRuntimeApi<Block>
        + StorageProvider<Block, BE>
        + Finalizer<Block, BE>
        + 'static,
    BE: Backend<Block>,
    Block: sp_runtime::traits::Block,
{
    spawner.spawn_essential_blocking(
        name,
        Some("flight-sql"),
        Box::pin(async move { run(client).await }),
    );
}

/// This function encapsulates the core logic of the flightsql task.
/// It is responsible for creating a FlightSQL Client and Subxt client. It listens
/// on the Subxt client for new blocks that have been finalized and responds to
/// data quorum and table creation events.
async fn run<Client, Block, BE>(chain_client: Arc<Client>)
where
    Client: BlockchainEvents<Block>
        + HeaderBackend<Block>
        + StorageProvider<Block, BE>
        + Finalizer<Block, BE>,
    BE: Backend<Block>,
    Block: sp_runtime::traits::Block,
{
    let client = Arc::new(Mutex::new(
        create_and_authenticate_default_flightsql().await.unwrap(),
    ));
    let api = create_subxt_client().await.unwrap();

    log::info!("FlightSQL: Task is running!");
    // Create the event stream
    let mut stream = chain_client.finality_notification_stream();
    while let Some(block) = stream.next().await {
        // Start by iterating through any blocks that were implicitly finalized
        let implicitly_finalized = block.tree_route;
        for hash in implicitly_finalized.iter() {
            let block = api
                .blocks()
                .at(subxt::utils::H256::from_slice(hash.as_ref()))
                .await
                .unwrap();

            log::info!(
                "FlightSQL: Processing implicitly finalized block {:?}",
                block.number()
            );

            process_block(&client, block)
                .await
                .expect("Unrecoverable FlightSQL Error; Please Verify Your DB Setup");
        }

        // Now process the latest finalized
        let hash = block.hash;
        let block = api
            .blocks()
            .at(subxt::utils::H256::from_slice(hash.as_ref()))
            .await
            .unwrap();

        // Process non-genesis blocks
        process_block(&client, block)
            .await
            .expect("Unrecoverable FlightSQL Error; Please Verify Your DB Setup");
    }
}

async fn process_block(
    client: &Arc<Mutex<FlightSqlServiceClient<Channel>>>,
    block: Block<PolkadotConfig, OnlineClient<PolkadotConfig>>,
) -> Result<(), SQLError> {
    log::info!("FlightSQL: Processing Block {:?}", block.number());

    let events = block
        .events()
        .await
        .expect("Failed to get events for finalized block")
        .iter()
        .filter_map(|maybe_event| {
            if let Ok(e) = maybe_event {
                Some(e)
            } else {
                None
            }
        });

    for event in events {
        // Check for a quorum being reached on submitted data
        if let Some(e) = event.as_event::<QuorumReached>().unwrap() {
            log::info!("FlightSQL: Processing Data Insert");
            let data = e.data;
            let id = identifier_to_sql(e.quorum.table.namespace.0, e.quorum.table.name.0)
                .expect("Corrupt table identifier!");
            log::info!("FlightSQL Task: Attempting insert to {id}");
            execute_with_backoff(
                |cli| {
                    let data = data.0.as_slice();
                    let id = id.as_str();
                    async move { insert_data(cli, data, id).await }
                },
                client.clone(),
            )
            .await
            .expect("Unrecoverable FlightSQL Error; Please Verify Your DB Setup");

        // Check for Schemas being updated (i.e. Table Creation)
        } else if let Some(e) = event.as_event::<SchemaUpdated>().unwrap() {
            log::info!("FlightSQL: Processing Table Creation");
            let raw_list: Vec<BoundedVec<u8>> =
                e.1 .0
                    .into_iter()
                    .map(|(update_table)| update_table.create_statement)
                    .collect();
            let list: Vec<&str> = raw_list
                .iter()
                .filter_map(|data| match from_utf8(data.0.as_slice()) {
                    Ok(sql) => Some(sql),
                    Err(_) => None,
                })
                .collect();
            log::info!("FlightSQL Task: Attempting Table Creation with {:?}", list);
            execute_with_backoff(
                |cli| {
                    let statement_slice = list.as_slice();

                    async move { create_tables(cli, statement_slice).await }
                },
                client.clone(),
            )
            .await
            .expect("Unrecoverable FlightSQL Error; Please Verify Your DB Setup");
        //Check for tables being created with commitments from a snapshot
        } else if let Some(e) = event.as_event::<TablesCreatedWithCommitments>().unwrap() {
            // TODO eventually parallelize this by wrapping the client in an Arc Mutex or similar
            log::info!("FlightSQL: Processing Table Creation With Snapshot");
            for req in e.table_list.0 {
                let sql = from_utf8(req.ddl.0.as_slice())
                    .expect("Genesis tables must have valid sql statements");
                let base_path = from_utf8(req.snapshot_url.0.as_slice())
                    .expect("Genesis table must have valid snapshot paths");
                let namespace = from_utf8(req.table_name.namespace.0.as_slice())
                    .expect("Genesis tables must have valid namespace")
                    .to_uppercase();
                log::info!(
                    "FlightSQL Task: Attempting table creation from genesis for {namespace}"
                );
                execute_with_backoff(|cli| {
                    let namespace = namespace.as_str();
                    async move { create_table_with_snapshot(cli, sql, base_path, namespace).await }
                }, client.clone()).await.expect("Loading historical data for genesis tables must succeed");
            }
        } else {
            continue;
        }
    }
    Ok(())
}

async fn create_and_authenticate_default_flightsql(
) -> Result<FlightSqlServiceClient<Channel>, anyhow::Error> {
    let flightsql_host = env::var("HOST").unwrap_or("127.0.0.1".into());
    let flightsql_port = env::var("PORT").unwrap_or("50555".into());
    let flightsql_user = env::var("FLIGHTSQL_USER").unwrap_or("admin".into());
    let flightsql_pass = env::var("FLIGHTSQL_PASSWORD").unwrap_or("admin".into());

    let endpoint = Channel::from_shared(format!("http://{flightsql_host}:{flightsql_port}"))?;
    let channel = endpoint.connect_lazy();

    // 20MB max message size
    let max_message_size: usize = 20 * 1024 * 1024;
    let inner = FlightServiceClient::new(channel).max_decoding_message_size(max_message_size);
    let mut client = FlightSqlServiceClient::new_from_inner(inner);
    client
        .handshake(flightsql_user.as_str(), flightsql_pass.as_str())
        .await?;
    Ok(client)
}

/// Create a subxt client to listen for blocks and events
async fn create_subxt_client() -> Result<OnlineClient<PolkadotConfig>, anyhow::Error> {
    let local_node_rpc = "ws://127.0.0.1:9944";

    // Build a custom WebSocket client so that we can apply our request and response size requirements
    let ws_client = RpcClient::builder()
        .max_request_size(50 * 1024 * 1024) // 50 Mb
        .max_response_size(50 * 1024 * 1024) // 50 Mb
        .request_timeout(Duration::from_secs(60))
        .connection_timeout(Duration::from_secs(10))
        .build(local_node_rpc.to_string())
        .await?;

    Ok(OnlineClient::<PolkadotConfig>::from_rpc_client(ws_client)
        .await
        .unwrap_or_else(|_| panic!("Unable to connect to local RPC at {local_node_rpc}!")))
}

/// Transforms Postcard Serialized OnChainTable into a RecordBatch
pub fn record_batch_from_data(on_chain_table_bytes: &[u8]) -> Result<RecordBatch, SQLError> {
    let table: OnChainTable = postcard::from_bytes(on_chain_table_bytes)
        .map_err(|e| SQLError::BadRecordBatch(e.to_string()))?;
    Ok(RecordBatch::from(table))
}

/// This helper function transforms data from a table identifier into a String representation compatible with
/// SQL statements
pub fn identifier_to_sql(namespace: Vec<u8>, name: Vec<u8>) -> Result<String, anyhow::Error> {
    let namespace = from_utf8(namespace.as_slice())?.to_uppercase();
    let name = from_utf8(name.as_slice())?.to_uppercase();
    Ok(format!("{namespace}.{name}"))
}

/// Create a schema for the supplied namespace
pub async fn create_schema_namespace(
    client: Arc<Mutex<FlightSqlServiceClient<Channel>>>,
    namespace: &str,
) -> Result<i64, arrow::error::ArrowError> {
    let mut client = client.lock().await;
    client
        .execute_update(format!("CREATE SCHEMA IF NOT EXISTS {namespace};"), None)
        .await
}

/// Create tables via SQL statements sent over FlightSQL
pub async fn create_tables(
    client: Arc<Mutex<FlightSqlServiceClient<Channel>>>,
    statement_list: &[&str],
) -> Result<(), arrow::error::ArrowError> {
    for sql in statement_list {
        let mut client = client.lock().await;
        client.execute_update(sql.to_string(), None).await?;
    }
    Ok(())
}

/// Create a new table and load existing historical data from a snapshot URL
pub async fn create_table_with_snapshot(
    client: Arc<Mutex<FlightSqlServiceClient<Channel>>>,
    sql: &str,
    snapshot_url: &str,
    namespace: &str,
) -> Result<(), arrow::error::ArrowError> {
    create_schema_namespace(client.clone(), namespace).await?;

    let mut client = client.lock().await;
    // First create the new table with FlightSQL
    client.execute_update(String::from(sql), None).await?;

    log::warn!("Skipping historical load for devnet!");
    Ok(())
}

/// Insert some data into FlightSQL via the RecordBatch API. Data is expected to tbe a
/// postcard serialized OnChainTable, identifier should be of the form "NAMESPACE.NAME"
pub async fn insert_data(
    client: Arc<Mutex<FlightSqlServiceClient<Channel>>>,
    data: &[u8],
    identifier: &str,
) -> Result<(), arrow::error::ArrowError> {
    let batch = record_batch_from_data(data)
        .map_err(|e| arrow::error::ArrowError::ParseError(format!("{:?}", e)))?;

    let batches = vec![batch];

    // Create the CommandStatementIngest object to be used in the ingestion process
    let cmd = CommandStatementIngest {
        table_definition_options: None,
        table: identifier.to_string(),
        schema: None,
        catalog: None,
        temporary: false,
        transaction_id: None,
        options: Default::default(),
    };

    let mut client = client.lock().await;
    let rows = client
        .execute_ingest(cmd, futures::stream::iter(batches.clone()).map(Ok))
        .await?;
    log::info!("FlightSQL: Inserted {:?}", rows);
    Ok(())
}

async fn execute_with_backoff<Fut, F>(
    mut call: F,
    client: Arc<Mutex<FlightSqlServiceClient<Channel>>>,
) -> Result<(), SQLError>
where
    F: FnMut(Arc<Mutex<FlightSqlServiceClient<Channel>>>) -> Fut,
    Fut: std::future::Future<Output = Result<(), arrow::error::ArrowError>>,
{
    for duration in exponential_backoff::Backoff::new(
        MAX_RETRY_ATTEMPTS,
        Duration::from_secs(MIN_DELAY_SECONDS),
        Duration::from_secs(MAX_DELAY_SECONDS),
    ) {
        match call(client.clone()).await {
            Ok(_) => {
                return Ok(());
            }
            Err(e) => {
                log::error!("Error with FlightSQL {:?}", e);

                if let ArrowError::IpcError(msg) = e {
                    if msg.contains("ERROR: duplicate key value violates unique constraint") {
                        log::warn!("FlightSQL Task: Encountered a duplicate key!");
                        return Ok(());
                    }

                    if msg.contains("code: Internal")
                        || msg.contains("code: Unavailable")
                        || msg.contains("status: Unavailable")
                    {
                        log::error!("FlightSQL Task: Attempting to reconnect to FlightSQL");
                        let maybe_client = create_and_authenticate_default_flightsql().await;

                        // Attempt a reconnect
                        match maybe_client {
                            Ok(new_client) => {
                                let mut cli = client.lock().await;
                                *cli = new_client;
                            }
                            Err(e) => {
                                log::error!("Error reconnecting with FlightSQL {:?}", e);
                            }
                        }
                    }
                };

                if let Some(duration) = duration {
                    log::error!("Retrying after {:?} delay in task", duration);
                    sleep(duration).await;
                    continue;
                }
            }
        }
    }
    Err(SQLError::FlightSQLServiceError(
        "Unable to recover from Error. Verify your DB Setup".to_string(),
    ))
}
