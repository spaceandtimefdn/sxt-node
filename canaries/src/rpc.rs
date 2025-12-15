use anyhow::Result;
use reqwest::Url;
use sxt_core::attestation::Attestation;
use sxt_core::keystore::H256;

/// Fetches Attestations via JSON RPC and returns the results, if any are found
pub(crate) async fn fetch_attestations(rpc_url: Url) -> Result<Vec<Attestation<H256>>> {
    let rpc_response: Vec<Attestation<H256>> = reqwest::Client::new()
        .post(rpc_url.clone())
        .json(&serde_json::json!({
            "id": 1,
            "method": "attestation_v1_bestRecentAttestations",
            "jsonrpc": "2.0",
        }))
        .send()
        .await?
        .json()
        .await?;

    Ok(rpc_response)
}
