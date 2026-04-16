//! HTTP+protobuf client for the indexer service.
//!
//! Uses `sp_io::offchain::http` to POST protobuf-encoded request bodies to
//! the indexer's HTTP endpoints. Works in both std and wasm (no_std) runtimes.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use prost::Message;
use sp_runtime::offchain::http::Request;
use sp_runtime::offchain::Duration;

use crate::proto;

/// Default HTTP request deadline (30 seconds).
const HTTP_TIMEOUT_MS: u64 = 30_000;

/// Errors from the HTTP client.
#[derive(Debug)]
pub enum Error {
    /// Failed to send the HTTP request.
    SendFailed,
    /// HTTP request timed out.
    DeadlineReached,
    /// IO or unknown error during HTTP request.
    IoError,
    /// Server returned a non-200 status code.
    Status(u16),
    /// Failed to decode the response body.
    Decode(prost::DecodeError),
}

impl From<prost::DecodeError> for Error {
    fn from(e: prost::DecodeError) -> Self {
        Error::Decode(e)
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::SendFailed => write!(f, "failed to send HTTP request"),
            Error::DeadlineReached => write!(f, "HTTP request deadline reached"),
            Error::IoError => write!(f, "HTTP IO error"),
            Error::Status(code) => write!(f, "server returned status {}", code),
            Error::Decode(e) => write!(f, "protobuf decode error: {}", e),
        }
    }
}

/// Fetch the last checkpoint from the server.
pub fn get_last_checkpoint(base_url: &str) -> Result<Option<u64>, Error> {
    let url = format!("{}/v1/get_last_checkpoint", base_url);
    let body = post(&url, &[])?;
    let resp = proto::GetLastCheckpointResponse::decode(body.as_slice())?;
    Ok(resp.has_checkpoint.then_some(resp.sequence_number))
}

/// Register a new table. v1 semantics: `arrow_schema` carries raw DDL bytes.
pub fn create_table(
    base_url: &str,
    sequence_number: u64,
    table_name: String,
    arrow_schema: Vec<u8>,
    key: String,
) -> Result<(), Error> {
    let req = proto::CreateTableRequest {
        sequence_number,
        table_name,
        arrow_schema,
        key,
    };
    let url = format!("{}/v1/create_table", base_url);
    post(&url, &req.encode_to_vec())?;
    Ok(())
}

/// Soft-delete a table.
pub fn drop_table(
    base_url: &str,
    sequence_number: u64,
    table_name: String,
) -> Result<(), Error> {
    let req = proto::DropTableRequest {
        sequence_number,
        table_name,
    };
    let url = format!("{}/v1/drop_table", base_url);
    post(&url, &req.encode_to_vec())?;
    Ok(())
}

/// Ingest one or more table batches.
pub fn put_batches(
    base_url: &str,
    sequence_number: u64,
    batches: Vec<proto::TableBatch>,
) -> Result<(), Error> {
    let req = proto::PutBatchesRequest {
        sequence_number,
        batches,
    };
    let url = format!("{}/v1/put_batches", base_url);
    post(&url, &req.encode_to_vec())?;
    Ok(())
}

/// Record a checkpoint for the given sequence number (block number).
pub fn checkpoint(base_url: &str, sequence_number: u64) -> Result<(), Error> {
    let req = proto::CheckpointRequest { sequence_number };
    let url = format!("{}/v1/checkpoint", base_url);
    post(&url, &req.encode_to_vec())?;
    Ok(())
}

/// Low-level POST helper. Sends `body` to `url` with
/// `Content-Type: application/x-protobuf` and returns the response body bytes.
fn post(url: &str, body: &[u8]) -> Result<Vec<u8>, Error> {
    let deadline = sp_io::offchain::timestamp().add(Duration::from_millis(HTTP_TIMEOUT_MS));

    // Pass body chunks as `Vec<Vec<u8>>`. Empty body → no chunks (otherwise
    // `Request::send` would write an empty chunk and then try to finalize
    // again, which fails with "body sending already finished").
    let chunks: Vec<Vec<u8>> = if body.is_empty() {
        Vec::new()
    } else {
        alloc::vec![body.to_vec()]
    };

    let pending = Request::post(url, chunks)
        .add_header("Content-Type", "application/x-protobuf")
        .deadline(deadline)
        .send()
        .map_err(|_| Error::SendFailed)?;

    let response = pending
        .try_wait(deadline)
        .map_err(|_| Error::DeadlineReached)?
        .map_err(|e| match e {
            sp_runtime::offchain::http::Error::DeadlineReached => Error::DeadlineReached,
            sp_runtime::offchain::http::Error::IoError => Error::IoError,
            sp_runtime::offchain::http::Error::Unknown => Error::IoError,
        })?;

    let status = response.code;
    if status != 200 {
        return Err(Error::Status(status));
    }

    Ok(response.body().collect())
}
