use anyhow::{anyhow, Result};
use reqwest::Url;
use serde::Deserialize;

/// JSON-RPC response wrapper
#[derive(Deserialize)]
struct JsonRpcResponse<T> {
    result: Option<T>,
    error: Option<JsonRpcError>,
}

#[derive(Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

/// Minimal attestation info needed for metrics.
/// We only need the address and block number for counting attestations.
#[derive(Clone)]
pub struct AttestationInfo {
    /// The 20-byte ethereum address as a hex string (with 0x prefix)
    pub address20: String,
    /// The block number that was attested
    #[expect(dead_code)]
    pub block_number: u32,
}

/// Local attestation type for deserializing RPC responses.
/// The sxt_core::attestation::Attestation type has asymmetric serde attributes
/// (serialize_with but no deserialize_with), so we define our own for deserialization.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RpcAttestation {
    address20: String, // hex string
    block_number: u32,
    // Other fields are ignored for metrics purposes
}

/// Response structure for bestRecentAttestations RPC
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AttestationsResponse {
    attestations: Vec<RpcAttestation>,
    // Other fields like attestations_for, attestations_for_block_number, at are ignored
}

/// Converts a WebSocket URL to an HTTP URL for RPC calls
fn ws_to_http(url: &Url) -> Result<Url> {
    let mut http_url = url.clone();
    match url.scheme() {
        "wss" => http_url
            .set_scheme("https")
            .map_err(|_| anyhow!("Failed to convert wss to https"))?,
        "ws" => http_url
            .set_scheme("http")
            .map_err(|_| anyhow!("Failed to convert ws to http"))?,
        "http" | "https" => {} // Already HTTP
        scheme => return Err(anyhow!("Unsupported URL scheme: {}", scheme)),
    }
    Ok(http_url)
}

/// Fetches Attestations via JSON RPC and returns the results, if any are found
pub(crate) async fn fetch_attestations(rpc_url: Url) -> Result<Vec<AttestationInfo>> {
    let http_url = ws_to_http(&rpc_url)?;

    let response: JsonRpcResponse<AttestationsResponse> = reqwest::Client::new()
        .post(http_url)
        .json(&serde_json::json!({
            "id": 1,
            "method": "attestation_v1_bestRecentAttestations",
            "jsonrpc": "2.0",
        }))
        .send()
        .await?
        .json()
        .await?;

    if let Some(error) = response.error {
        return Err(anyhow!("RPC error {}: {}", error.code, error.message));
    }

    response
        .result
        .map(|r| {
            r.attestations
                .into_iter()
                .map(|a| AttestationInfo {
                    address20: a.address20,
                    block_number: a.block_number,
                })
                .collect()
        })
        .ok_or_else(|| anyhow!("RPC response missing result field"))
}
