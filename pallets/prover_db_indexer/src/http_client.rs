//! HTTP+protobuf client for the indexer service.
//!
//! Uses `polkadot_sdk::sp_io::offchain::http` to POST protobuf-encoded request bodies to
//! the indexer's HTTP endpoints. Works in both std and wasm (no_std) runtimes.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use polkadot_sdk::sp_runtime::offchain::http::Request;
use polkadot_sdk::sp_runtime::offchain::Duration;
use prost::Message;
use snafu::Snafu;
use sxt_core::tables::TableIdentifier;
use url::Url;

use crate::proto;

/// Default HTTP request deadline (30 seconds).
const HTTP_TIMEOUT_MS: u64 = 30_000;

// Relative path components for the indexer's v1 HTTP API. Kept relative
// (no leading `/`) so `Url::join` appends them rather than replacing the
// base URL's path; see `endpoint` for details.
const PATH_GET_LAST_CHECKPOINT: &str = "v1/get_last_checkpoint";
const PATH_CREATE_TABLE: &str = "v1/create_table";
const PATH_DROP_TABLE: &str = "v1/drop_table";
const PATH_PUT_BATCHES: &str = "v1/put_batches";
const PATH_CHECKPOINT: &str = "v1/checkpoint";

/// Errors from the HTTP client.
///
/// The `Status` variant's display formats up to ~256 bytes of the response
/// body as UTF-8 lossy so the operator can read whatever the server said
/// (often a plain-text reason for 4xx/5xx).
#[derive(Debug, Snafu)]
pub enum Error {
    /// Failed to send the HTTP request.
    #[snafu(display("failed to send HTTP request"))]
    SendFailed,
    /// HTTP request timed out.
    #[snafu(display("HTTP request deadline reached"))]
    DeadlineReached,
    /// IO or unknown error during HTTP request.
    #[snafu(display("HTTP IO error"))]
    Io,
    /// Server returned a non-200 status code. Carries the response body
    /// so the caller can surface whatever the server said about the failure.
    #[snafu(display(
        "server returned status {} ({} body bytes): {}",
        code,
        body.len(),
        core::str::from_utf8(&body[..core::cmp::min(body.len(), 256)]).unwrap_or("<non-utf8>"),
    ))]
    Status { code: u16, body: Vec<u8> },
    /// Failed to decode the response body.
    #[snafu(display("protobuf decode error: {error}"))]
    Decode {
        /// Not named `source` because `prost::DecodeError` doesn't implement
        /// `core::error::Error` in no_std mode, which would break Snafu's
        /// derived source-chain wiring in the wasm runtime build.
        error: prost::DecodeError,
    },
}

impl From<prost::DecodeError> for Error {
    fn from(error: prost::DecodeError) -> Self {
        Error::Decode { error }
    }
}

/// Resolve `path` against `base_url` per RFC 3986. The path constants
/// below are all relative ("v1/..."), so this always appends them; if an
/// operator configures a base with a subpath, make sure it ends in `/`
/// or `Url::join` will replace that last segment.
fn endpoint(base_url: &Url, path: &str) -> String {
    base_url
        .join(path)
        .expect("constant relative path always joins against a valid base URL")
        .to_string()
}

/// Fetch the last checkpoint from the server.
pub fn get_last_checkpoint(base_url: &Url) -> Result<Option<u64>, Error> {
    let url = endpoint(base_url, PATH_GET_LAST_CHECKPOINT);
    let body = post(&url, &[])?;
    let resp = proto::GetLastCheckpointResponse::decode(body.as_slice())?;
    Ok(resp.has_checkpoint.then_some(resp.sequence_number))
}

/// Register a new table. v1 semantics: `arrow_schema` carries raw DDL bytes.
pub fn create_table(
    base_url: &Url,
    sequence_number: u64,
    table: &TableIdentifier,
    arrow_schema: Vec<u8>,
    key: String,
) -> Result<(), Error> {
    let req = proto::CreateTableRequest {
        sequence_number,
        table_name: table.to_string(),
        arrow_schema,
        key,
    };
    let url = endpoint(base_url, PATH_CREATE_TABLE);
    post(&url, &req.encode_to_vec())?;
    Ok(())
}

/// Soft-delete a table.
pub fn drop_table(
    base_url: &Url,
    sequence_number: u64,
    table: &TableIdentifier,
) -> Result<(), Error> {
    let req = proto::DropTableRequest {
        sequence_number,
        table_name: table.to_string(),
    };
    let url = endpoint(base_url, PATH_DROP_TABLE);
    post(&url, &req.encode_to_vec())?;
    Ok(())
}

/// Ingest one or more table batches. Each `(table, record_batch)` pair
/// is wrapped in a `proto::TableBatch` and sent under the given
/// `sequence_number`.
pub fn put_batches(
    base_url: &Url,
    sequence_number: u64,
    batches: Vec<(&TableIdentifier, Vec<u8>)>,
) -> Result<(), Error> {
    let req = proto::PutBatchesRequest {
        sequence_number,
        batches: batches
            .into_iter()
            .map(|(table, record_batch)| proto::TableBatch {
                table_name: table.to_string(),
                record_batch,
            })
            .collect(),
    };
    let url = endpoint(base_url, PATH_PUT_BATCHES);
    post(&url, &req.encode_to_vec())?;
    Ok(())
}

/// Record a checkpoint for the given sequence number (block number).
pub fn checkpoint(base_url: &Url, sequence_number: u64) -> Result<(), Error> {
    let req = proto::CheckpointRequest { sequence_number };
    let url = endpoint(base_url, PATH_CHECKPOINT);
    post(&url, &req.encode_to_vec())?;
    Ok(())
}

/// Low-level POST helper. Sends `body` to `url` with
/// `Content-Type: application/x-protobuf` and returns the response body bytes.
fn post(url: &str, body: &[u8]) -> Result<Vec<u8>, Error> {
    polkadot_sdk::sp_tracing::debug!(
        target: "prover_db_indexer",
        "POST {} ({} body bytes)",
        url,
        body.len(),
    );

    let deadline =
        polkadot_sdk::sp_io::offchain::timestamp().add(Duration::from_millis(HTTP_TIMEOUT_MS));

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
        .map_err(|e| {
            polkadot_sdk::sp_tracing::warn!(
                target: "prover_db_indexer",
                "POST {} send failed: {:?}",
                url, e,
            );
            Error::SendFailed
        })?;

    let response = pending
        .try_wait(deadline)
        .map_err(|e| {
            polkadot_sdk::sp_tracing::warn!(
                target: "prover_db_indexer",
                "POST {} try_wait failed: {:?}",
                url, e,
            );
            Error::DeadlineReached
        })?
        .map_err(|e| {
            polkadot_sdk::sp_tracing::warn!(
                target: "prover_db_indexer",
                "POST {} response error: {:?}",
                url, e,
            );
            match e {
                polkadot_sdk::sp_runtime::offchain::http::Error::DeadlineReached => {
                    Error::DeadlineReached
                }
                polkadot_sdk::sp_runtime::offchain::http::Error::IoError => Error::Io,
                polkadot_sdk::sp_runtime::offchain::http::Error::Unknown => Error::Io,
            }
        })?;

    let status = response.code;
    let response_body: Vec<u8> = response.body().collect();

    polkadot_sdk::sp_tracing::debug!(
        target: "prover_db_indexer",
        "POST {} -> status {} ({} body bytes)",
        url, status, response_body.len(),
    );

    if status != 200 {
        // Log the response body inline at warn so it shows up without
        // needing debug filters — 4xx/5xx bodies usually carry the
        // server's reason (route not registered, bad payload, etc.).
        let snippet_len = core::cmp::min(response_body.len(), 256);
        let snippet = core::str::from_utf8(&response_body[..snippet_len]).unwrap_or("<non-utf8>");
        polkadot_sdk::sp_tracing::warn!(
            target: "prover_db_indexer",
            "POST {} -> status {}; body ({} bytes): {}",
            url, status, response_body.len(), snippet,
        );
        return Err(Error::Status {
            code: status,
            body: response_body,
        });
    }

    Ok(response_body)
}
