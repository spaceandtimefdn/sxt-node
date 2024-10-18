use core::str::from_utf8;
use std::env;
use std::time::Duration;

use arrow::record_batch::RecordBatch;
use arrow_flight::encode::FlightDataEncoderBuilder;
use arrow_flight::flight_service_client::FlightServiceClient;
use arrow_flight::sql::client::FlightSqlServiceClient;
use arrow_flight::FlightDescriptor;
use frame_support::__private::log;
use on_chain_table::OnChainTable;
use sp_core::traits::SpawnEssentialNamed;
use sp_runtime_interface::sp_wasm_interface::anyhow;
use subxt::backend::rpc::reconnecting_rpc_client::Client;
use subxt::ext::futures;
use subxt::ext::futures::StreamExt;
use subxt::{OnlineClient, PolkadotConfig};
use tonic::transport::Channel;
#[cfg(not(doctest))] // Skip doc tests on generated file
use {
    crate::sxt_chain_runtime::api::indexing::events::QuorumReached,
    crate::sxt_chain_runtime::api::runtime_types::bounded_collections::bounded_vec::BoundedVec,
    crate::sxt_chain_runtime::api::tables::events::SchemaUpdated,
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
pub fn spawn_flightsql_tasks(name: &'static str, spawner: &impl SpawnEssentialNamed) {
    spawner.spawn_essential_blocking(
        name,
        Some("flight-sql"),
        Box::pin(async move { run().await }),
    );
}

/// This function encapsulates the core logic of the flightsql task.
/// It is responsible for creating a FlightSQL Client and Subxt client. It listens
/// on the Subxt client for new blocks that have been finalized and responds to
/// data quorum and table creation events.
async fn run() {
    let flightsql_host = env::var("HOST").unwrap_or("127.0.0.1".into());
    let flightsql_port = env::var("PORT").unwrap_or("50555".into());

    let mut client = create_flightsql_client(&flightsql_host, &flightsql_port).await.unwrap_or_else(|_|
        panic!("Unable to connect to flightSQL at {flightsql_host}:{flightsql_port}! FlightSQL is required for all validators!")
    );

    let api = create_subxt_client().await.unwrap();

    let mut block_stream = api
        .blocks()
        .subscribe_finalized()
        .await
        .expect("Unable to subscribe to block finalization");

    // Essential tasks can't exit or the node stops
    while let Some(block) = block_stream.next().await {
        let events = block
            .unwrap()
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
            let result;
            if let Some(e) = event.as_event::<QuorumReached>().unwrap() {
                let data = e.data;
                let inner = client.inner_mut();
                let id = identifier_to_sql(e.quorum.table.namespace.0, e.quorum.table.name.0)
                    .expect("Corrupt table identifier!");
                result = insert_data(inner, data.0, id).await;
            } else if let Some(e) = event.as_event::<SchemaUpdated>().unwrap() {
                let raw_list: Vec<BoundedVec<u8>> =
                    e.1 .0.into_iter().map(|(_, statement)| statement).collect();
                let list: Vec<String> = raw_list
                    .iter()
                    .filter_map(|data| match from_utf8(data.0.as_slice()) {
                        Ok(sql) => Some(sql.to_string()),
                        Err(_) => None,
                    })
                    .collect();

                result = create_tables(&mut client, list).await;
            } else {
                continue;
            }

            match result {
                Ok(_) => {}
                Err(e) => match e {
                    SQLError::DBServiceError(msg) => {
                        log::error!("ERROR DBServiceError {msg}")
                    }
                    SQLError::FlightSQLServiceError(msg) => {
                        log::error!("ERROR FlightSQLServiceError {msg}")
                    }
                    SQLError::BadTableIdentifier(msg) => {
                        log::error!("ERROR BadTableIdentifier {msg}")
                    }
                    SQLError::BadSQLStatement(msg) => {
                        log::error!("ERROR BadSQLStatement {msg}")
                    }
                    SQLError::SQLExecutionError(msg) => {
                        log::error!("ERROR SQLExecutionError {msg}")
                    }
                    SQLError::InsertExecutionError(msg) => {
                        log::error!("ERROR InsertExecutionError {msg}")
                    }
                    SQLError::BadRecordBatch(msg) => {
                        log::error!("ERROR BadRecordBatch {msg}")
                    }
                },
            }
        }
    }
}

/// Create a FlightSQL client to interact with the SQL database
async fn create_flightsql_client(
    host: &str,
    port: &str,
) -> Result<FlightSqlServiceClient<Channel>, anyhow::Error> {
    let endpoint = Channel::from_shared(format!("http://{host}:{port}"))?;
    let channel = endpoint.connect().await?;
    Ok(FlightSqlServiceClient::new(channel))
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

/// Create tables via SQL statements sent over FlightSQL
pub async fn create_tables(
    client: &mut FlightSqlServiceClient<Channel>,
    statement_list: Vec<String>,
) -> Result<(), SQLError> {
    for sql in statement_list {
        client
            .execute(sql, None)
            .await
            .map_err(|e| SQLError::SQLExecutionError(e.to_string()))?;
    }
    Ok(())
}

/// Insert some data into FlightSQL via the RecordBatch API. Data is expected to tbe a
/// postcard serialized OnChainTable, identifier should be of the form "NAMESPACE.NAME"
pub async fn insert_data(
    client: &mut FlightServiceClient<Channel>,
    data: Vec<u8>,
    identifier: String,
) -> Result<(), SQLError> {
    let descriptor = FlightDescriptor::new_path(vec![identifier]);

    let batch = record_batch_from_data(data)?;

    // Create an input stream of RecordBatch
    let input_stream = futures::stream::iter(vec![batch].into_iter().map(Ok));

    // Encode the input stream with the table identifier via a FlightData Descriptor
    let flight_data_stream = FlightDataEncoderBuilder::new()
        .with_flight_descriptor(Some(descriptor))
        .build(input_stream);

    let flight_data_stream = flight_data_stream.map(|result| match result {
        Ok(flight_data) => flight_data,
        Err(e) => {
            // Handle the error appropriately
            // You can log the error and return an empty FlightData, or you can stop the stream
            // For this example, we'll return an error via panic (not recommended for production)
            panic!("Error encoding FlightData: {:?}", e);
        }
    });

    // Wrap the data stream in a new request object
    let request = tonic::Request::new(flight_data_stream);

    // Submit the request and await the result
    let response = client
        .do_put(request)
        .await
        .map_err(|e| SQLError::InsertExecutionError(e.message().to_string()));

    println!("Client: Server responded with {:?}", response);
    Ok(())
}
