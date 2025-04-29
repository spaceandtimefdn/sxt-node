use std::sync::Arc;
use std::time::Duration;

use anyhow::Error;
use log::info;
use subxt::backend::rpc::reconnecting_rpc_client::RpcClient;
use subxt::utils::AccountId32;
use subxt::{OnlineClient, PolkadotConfig};
use sxt_core::sxt_chain_runtime;
use tokio::sync::Mutex;
use url::Url;

/// Create the subxt client
pub(crate) async fn create_subxt_client(
    rpc_url: &Url,
) -> Result<Arc<Mutex<OnlineClient<PolkadotConfig>>>, Error> {
    info!("Connecting to Substrate node at: {}", rpc_url);

    let ws_client = RpcClient::builder()
        .max_request_size(50 * 1024 * 1024)
        .max_response_size(50 * 1024 * 1024)
        .request_timeout(Duration::from_secs(60))
        .connection_timeout(Duration::from_secs(10))
        .build(rpc_url.to_string())
        .await?;

    info!("Substrate client connected");
    Ok(Arc::new(Mutex::new(
        OnlineClient::<PolkadotConfig>::from_rpc_client(ws_client).await?,
    )))
}

/// Get the currently expected nonce for our account according to the chain.
/// We use this as our starting point
pub(crate) async fn get_starting_nonce(
    api: Arc<Mutex<OnlineClient<PolkadotConfig>>>,
    id: AccountId32,
) -> u64 {
    // We need to get the current nonce for the account as a starting point
    let account_info = api
        .lock()
        .await
        .storage()
        .at_latest()
        .await
        .unwrap()
        .fetch(&sxt_chain_runtime::api::storage().system().account(&id))
        .await
        .unwrap();

    // Extract the nonce
    match account_info {
        None => 0u64,
        Some(info) => info.nonce as u64,
    }
}
