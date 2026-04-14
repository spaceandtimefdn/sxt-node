//! Mock HTTP server that mirrors the indexer gRPC service's proto, but over
//! HTTP + protobuf. Used for local testing of the offchain indexing pallet.
//!
//! Endpoints:
//!   POST /v1/create_table        — body: CreateTableRequest (protobuf)
//!   POST /v1/drop_table          — body: DropTableRequest (protobuf)
//!   POST /v1/put_batches         — body: PutBatchesRequest (protobuf)
//!   POST /v1/checkpoint          — body: CheckpointRequest (protobuf)
//!   POST /v1/get_last_checkpoint — empty body, response: GetLastCheckpointResponse (protobuf)
//!
//! State is held in memory (not persisted). The server enforces the
//! `sequence_number == last_checkpoint + 1` rule.

use std::sync::{Arc, Mutex};

use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::Router;
use prost::Message;

mod proto {
    include!(concat!(env!("OUT_DIR"), "/io.spaceandtime.indexer.rs"));
}

/// In-memory server state.
#[derive(Debug, Default)]
struct ServerState {
    /// Last checkpointed sequence number.
    last_checkpoint: Option<u64>,
    /// Known tables (by fully qualified name).
    tables: std::collections::HashSet<String>,
}

type SharedState = Arc<Mutex<ServerState>>;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let state: SharedState = Arc::new(Mutex::new(ServerState::default()));

    let app = Router::new()
        .route("/v1/create_table", post(create_table))
        .route("/v1/drop_table", post(drop_table))
        .route("/v1/put_batches", post(put_batches))
        .route("/v1/checkpoint", post(checkpoint))
        .route("/v1/get_last_checkpoint", post(get_last_checkpoint))
        .with_state(state);

    let addr = "0.0.0.0:9999";
    tracing::info!("mock indexer server listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn create_table(
    State(state): State<SharedState>,
    body: Bytes,
) -> Result<(), StatusCode> {
    let req = proto::CreateTableRequest::decode(body)
        .map_err(|e| {
            tracing::error!("decode error: {}", e);
            StatusCode::BAD_REQUEST
        })?;

    tracing::info!(
        seq = req.sequence_number,
        table = %req.table_name,
        key = %req.key,
        schema_len = req.arrow_schema.len(),
        "CreateTable",
    );

    let mut s = state.lock().unwrap();
    if s.tables.contains(&req.table_name) {
        tracing::warn!(table = %req.table_name, "table already exists");
        // Return 409 Conflict (maps to ALREADY_EXISTS)
        return Err(StatusCode::CONFLICT);
    }
    s.tables.insert(req.table_name);
    Ok(())
}

async fn drop_table(
    State(state): State<SharedState>,
    body: Bytes,
) -> Result<(), StatusCode> {
    let req = proto::DropTableRequest::decode(body)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    tracing::info!(
        seq = req.sequence_number,
        table = %req.table_name,
        "DropTable",
    );

    let mut s = state.lock().unwrap();
    s.tables.remove(&req.table_name);
    Ok(())
}

async fn put_batches(body: Bytes) -> Result<(), StatusCode> {
    let req = proto::PutBatchesRequest::decode(body)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    for batch in &req.batches {
        tracing::info!(
            seq = req.sequence_number,
            table = %batch.table_name,
            batch_len = batch.record_batch.len(),
            "PutBatches",
        );
    }
    Ok(())
}

async fn checkpoint(
    State(state): State<SharedState>,
    body: Bytes,
) -> Result<(), StatusCode> {
    let req = proto::CheckpointRequest::decode(body)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let mut s = state.lock().unwrap();
    let expected = s.last_checkpoint.map_or(1, |n| n + 1);

    // Idempotent: accept if already at this checkpoint.
    if s.last_checkpoint == Some(req.sequence_number) {
        tracing::debug!(seq = req.sequence_number, "checkpoint idempotent — already at this seq");
        return Ok(());
    }

    if req.sequence_number != expected {
        tracing::error!(
            seq = req.sequence_number,
            expected = expected,
            "checkpoint sequence mismatch",
        );
        return Err(StatusCode::BAD_REQUEST);
    }

    s.last_checkpoint = Some(req.sequence_number);
    tracing::info!(seq = req.sequence_number, "Checkpoint");
    Ok(())
}

async fn get_last_checkpoint(
    State(state): State<SharedState>,
) -> Result<Bytes, StatusCode> {
    let s = state.lock().unwrap();
    let resp = proto::GetLastCheckpointResponse {
        sequence_number: s.last_checkpoint.unwrap_or(0),
        has_checkpoint: s.last_checkpoint.is_some(),
    };
    Ok(Bytes::from(resp.encode_to_vec()))
}
