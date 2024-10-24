use core::str::from_utf8;
use std::env;
use std::hash::Hash;
use std::sync::Arc;
use sc_client_api::BlockchainEvents;
use sp_blockchain::HeaderBackend;
use sp_api::ProvideRuntimeApi;
use tokio::sync::Mutex;
use std::time::Duration;

use arrow::record_batch::RecordBatch;
use arrow_flight::encode::FlightDataEncoderBuilder;
use arrow_flight::flight_service_client::FlightServiceClient;
use arrow_flight::sql::client::FlightSqlServiceClient;
use arrow_flight::sql::CommandStatementIngest;
use arrow_flight::FlightDescriptor;
use frame_support::__private::log;
use sp_core::H256;
use on_chain_table::OnChainTable;
use sp_core::traits::SpawnEssentialNamed;
use sp_runtime_interface::sp_wasm_interface::anyhow;
use sp_runtime_interface::sp_wasm_interface::anyhow::Error;
use subxt::backend::rpc::reconnecting_rpc_client::Client;
use subxt::ext::futures;
use subxt::ext::futures::{Stream, StreamExt};
use subxt::{OnlineClient, PolkadotConfig};
use subxt::blocks::Block;
use subxt::client::OfflineClientT;
use tonic::transport::Channel;
#[cfg(not(doctest))] // Skip doc tests on generated file
use {
    crate::sxt_chain_runtime::api::indexing::events::QuorumReached,
    crate::sxt_chain_runtime::api::runtime_types::bounded_collections::bounded_vec::BoundedVec,
    crate::sxt_chain_runtime::api::system::events::ExtrinsicSuccess,
    crate::sxt_chain_runtime::api::tables::events::SchemaUpdated,
    crate::sxt_chain_runtime::api::tables::events::TablesCreatedWithCommitments,
};

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

/// Wrapper to spawn the flightsql task using the provided Spawn handle.
pub fn spawn_flightsql_tasks<Client, Block>(name: &'static str, spawner: &impl SpawnEssentialNamed, client: Arc<Client>)
where
    Client: BlockchainEvents<Block> + HeaderBackend<Block> + ProvideRuntimeApi<Block> + 'static,
    Block: sp_runtime::traits::Block, {
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
async fn run<Client, Block>(chain_client: Arc<Client>) where
    Client: BlockchainEvents<Block> + HeaderBackend<Block>,
    Block: sp_runtime::traits::Block  {

    let flightsql_host = env::var("HOST").unwrap_or("127.0.0.1".into());
    let flightsql_port = env::var("PORT").unwrap_or("50555".into());
    let flightsql_user = env::var("FLIGHTSQL_USER").unwrap_or("admin".into());
    let flightsql_pass = env::var("FLIGHTSQL_PASSWORD").unwrap_or("admin".into());

    let client = create_flightsql_client(&flightsql_host, &flightsql_port).await.unwrap_or_else(|_|
        panic!("Unable to connect to flightSQL at {flightsql_host}:{flightsql_port}! FlightSQL is required for all validators!")
    );

    authenticate_client(&client, &flightsql_user, &flightsql_pass)
        .await
        .unwrap();

    let api = create_subxt_client().await.unwrap();

    log::info!("FlightSQL: Task is running!");
    // Create the event stream
    let mut stream = chain_client.finality_notification_stream();
    while let Some(block) = stream.next().await {
        let hash = block.hash;
        let block = api.blocks().at(H256::from_slice(hash.as_ref())).await.unwrap();

        // The genesis block is skipped, so if we're getting block 1 it's because we just missed genesis
        // We need to request the genesis block and its events specifically

        if block.number() == 1 {
            let genesis = api.blocks().at(api.genesis_hash()).await.unwrap();

            log::info!("FlightSQL: Processing GENESIS {:?}", genesis.hash());
            let _ = process_block(&client, genesis).await.unwrap();
        }
        log::info!("FlightSQL: Processing Block {:?}", block.number());

        let result = process_block(&client, block).await;
        match result {
            Ok(_) => {}
            Err(e) => {
                log::error!("FlightSQL: Error {:?}", e);
            }
        }
    }
}

async fn process_block(mut client: &Arc<Mutex<FlightSqlServiceClient<Channel>>>, block: Block<PolkadotConfig, OnlineClient<PolkadotConfig>>) -> Result<(), SQLError> {
    let mut client = client.lock().await;
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

    Ok(for event in events {
        let result;
        // Check for a quorum being reached on submitted data
        if let Some(e) = event.as_event::<QuorumReached>().unwrap() {
            log::info!("FlightSQL: Processing Data Insert");
            let data = e.data;
            let id = identifier_to_sql(e.quorum.table.namespace.0, e.quorum.table.name.0)
                .expect("Corrupt table identifier!");
            result = insert_data(&mut client, data.0, id).await;

        // Check for Schemas being updated (i.e. Table Creation)
        } else if let Some(e) = event.as_event::<SchemaUpdated>().unwrap() {
            log::info!("FlightSQL: Processing Table Creation");
            let raw_list: Vec<BoundedVec<u8>> =
                e.1.0.into_iter().map(|(_, statement)| statement).collect();
            let list: Vec<String> = raw_list
                .iter()
                .filter_map(|data| match from_utf8(data.0.as_slice()) {
                    Ok(sql) => Some(sql.to_string()),
                    Err(_) => None,
                })
                .collect();

            result = create_tables(&mut client, list).await;
        //Check for tables being created with commitments from a snapshot
        } else if let Some(e) = event.as_event::<TablesCreatedWithCommitments>().unwrap() {
            // TODO eventually parallelize this by wrapping the client in an Arc Mutex or similar
            log::info!("FlightSQL: Processing Table Creation With Snapshot");
            for (id, sql, c, base_path) in e.table_list.0 {
                let sql = from_utf8(sql.0.as_slice())
                    .expect("Genesis tables must have valid sql statements");
                let base_path = from_utf8(base_path.0.as_slice())
                    .expect("Genesis table must have valid snapshot paths");
                let namespace = from_utf8(id.namespace.0.as_slice())
                    .expect("Genesis tables must have valid namespace")
                    .to_uppercase();
                create_table_with_snapshot(
                    &mut client,
                    sql.to_string(),
                    base_path,
                    &namespace
                )
                    .await
                    .expect("Loading historical data for genesis tables must succeed");
            }
            result = Ok(())
        } else {
            continue;
        }
    })
}


/// Create a FlightSQL client to interact with the SQL database
async fn create_flightsql_client(
    host: &str,
    port: &str,
) -> Result<Arc<Mutex<FlightSqlServiceClient<Channel>>>, anyhow::Error> {
    let endpoint = Channel::from_shared(format!("http://{host}:{port}"))?;
    let channel = endpoint.connect().await?;
    Ok(Arc::new(Mutex::new(FlightSqlServiceClient::new(channel))))
}

/// Authenticate with the flightsql server using the provided username and password
async fn authenticate_client(
    client: &Arc<Mutex<FlightSqlServiceClient<Channel>>>,
    user: &str,
    pass: &str,
) -> Result<(), anyhow::Error> {
    let mut c = client.lock().await;
    c.set_header("SUBSCRIPTION_ID", "subscription-id");
    let _ = c.handshake(user, pass).await?;
    Ok(())
}

/// Create a subxt client to listen for blocks and events
async fn create_subxt_client() -> Result<OnlineClient<PolkadotConfig>, anyhow::Error> {
    let local_node_rpc = "ws://127.0.0.1:9944";

    // Build a custom WebSocket client so that we can apply our request and response size requirements
    let ws_client = Client::builder()
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
pub fn record_batch_from_data(on_chain_table_bytes: Vec<u8>) -> Result<RecordBatch, SQLError> {
    let table: OnChainTable = postcard::from_bytes(on_chain_table_bytes.as_slice())
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
pub async fn create_schema_namespace(client: &mut FlightSqlServiceClient<Channel>, namespace: &str) -> Result<i64, SQLError> {
    client.execute_update(format!("CREATE SCHEMA IF NOT EXISTS {namespace};"), None).await.map_err(|e| SQLError::SQLExecutionError(e.to_string()))
}

/// Create tables via SQL statements sent over FlightSQL
pub async fn create_tables(
    client: &mut FlightSqlServiceClient<Channel>,
    statement_list: Vec<String>,
) -> Result<(), SQLError> {
    for sql in statement_list {
        client
            .execute_update(sql, None)
            .await
            .map_err(|e| SQLError::SQLExecutionError(e.to_string()))?;
    }
    Ok(())
}

/// Create a new table and load existing historical data from a snapshot URL
pub async fn create_table_with_snapshot(
    client: &mut FlightSqlServiceClient<Channel>,
    sql: String,
    snapshot_url: &str,
    namespace: &str,
) -> Result<(), SQLError> {
    create_schema_namespace(client, namespace).await?;

    // First create the new table with FlightSQL
    client
        .execute_update(sql, None)
        .await
        .map_err(|e| SQLError::SQLExecutionError(e.to_string()))?;

    // Start the historical data load into the table
    #[allow(clippy::identity_op)]
    let one_hour_in_seconds = 60 * 60 * 1;
    match data_loader::data_loader::run_data_loader(
        snapshot_url,
        5,
        Duration::from_secs(one_hour_in_seconds),
    )
    .await
    {
        Ok(_) => Ok(()),
        Err(e) => Err(SQLError::DBServiceError(e.to_string())),
    }
}

/// Insert some data into FlightSQL via the RecordBatch API. Data is expected to tbe a
/// postcard serialized OnChainTable, identifier should be of the form "NAMESPACE.NAME"
pub async fn insert_data(
    client: &mut FlightSqlServiceClient<Channel>,
    data: Vec<u8>,
    identifier: String,
) -> Result<(), SQLError> {
    let batch = record_batch_from_data(data)?;

    let batches = vec![batch];

    // Create the CommandStatementIngest object to be used in the ingestion process
    let cmd = CommandStatementIngest {
        table_definition_options: None,
        table: identifier.clone(),
        schema: None,
        catalog: None,
        temporary: false,
        transaction_id: None,
        options: Default::default(),
    };

    // Execute the ingestion and assert that the number of rows ingested is correct
    let actual_rows = client
        .execute_ingest(cmd, futures::stream::iter(batches.clone()).map(Ok))
        .await
        .map_err(|e| SQLError::InsertExecutionError(e.to_string()))?;

    log::info!("FlightSQL: Inserted {:?} rows to {identifier}", actual_rows);
    Ok(())
}