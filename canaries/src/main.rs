//! Canary prototype
/// Prometheus metrics collection and serving.
mod metrics;
/// Parsing utilities for canary data.
mod parsing;
/// RPC communication utilities.
mod rpc;
/// Storage operations for canary state.
mod storage;
use std::net::SocketAddr;

use clap::Parser;
use env_logger::Env;
use event_forwarder::block_processing::fetch_all_events;
use event_forwarder::chain_listener::{
    Block,
    BlockProcessor,
    ChainListener,
    FinalizedBlockStream,
    API,
};
use log::info;
use snafu::Snafu;
use subxt::utils::AccountId32;
use subxt::{OnlineClient, PolkadotConfig};
use url::Url;

use crate::metrics::*;
use crate::parsing::*;
use crate::storage::*;

/// Canary: Substrate Finalized Block Event Monitor
#[derive(Debug, Parser)]
#[command(name = "canary")]
#[command(author = "Your Team")]
#[command(version = "0.1.0")]
#[command(about = "Watches finalized Substrate blocks and counts specific events", long_about = None)]
pub struct CanaryConfig {
    /// WebSocket URL of the Substrate node
    #[arg(
        long,
        env = "CANARY_RPC_URL",
        default_value = "wss://new-rpc.testnet.sxt.network"
    )]
    pub rpc_url: Url,

    /// Bind address for Prometheus metrics (e.g., 0.0.0.0:9000)
    #[arg(long, env = "CANARY_METRICS_BIND", default_value = "0.0.0.0:9000")]
    pub metrics_bind: SocketAddr,

    /// List of account IDs for balance monitoring in SS58 format
    #[arg(
        long,
        env = "CANARY_WATCHLIST_IDS",
        value_delimiter = ',',
        num_args = 0..
    )]
    pub watchlist_ids: Vec<AccountId32>,
}

#[tokio::main]
async fn main() -> Result<(), CanaryError> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    info!("🚀 Starting Canary block listener...");

    let config = CanaryConfig::parse();

    // Start metrics server
    tokio::spawn({
        let addr = config.metrics_bind;
        async move {
            if let Err(e) = serve_metrics(addr).await {
                log::error!("❌ Failed to start metrics server: {:?}", e);
            }
        }
    });

    // Connect to the Substrate node
    let api = OnlineClient::<PolkadotConfig>::from_insecure_url(&config.rpc_url).await?;

    // Set up the listener
    let stream = FinalizedBlockStream;
    let processor = SimpleProcessor {
        rpc_url: config.rpc_url.clone(),
        watchlist: config.watchlist_ids,
    };

    let listener = ChainListener::new(processor, stream, api)
        .await
        .map_err(|e| CanaryError::ChainSetup { source: e })?;

    listener.run().await;

    Ok(())
}

/// Simple block processor for canary monitoring.
struct SimpleProcessor {
    /// The RPC URL to connect to.
    pub rpc_url: Url,

    /// A list of account IDs for balance monitoring
    pub watchlist: Vec<AccountId32>,
}

#[async_trait::async_trait]
impl BlockProcessor for SimpleProcessor {
    async fn process_block(&mut self, api: &API, block: Block) {
        let events = fetch_all_events(&block).await.unwrap_or_default();

        // Parse the event names and record each one
        parse_event_names(&events)
            .iter()
            .for_each(|name| record_event(name));

        parse_staking_stats(&events).iter().for_each(record_staking);

        parse_balance_stats(&events).iter().for_each(record_balance);

        match rpc::fetch_attestations(self.rpc_url.clone()).await {
            Ok(attestations) => record_attestations(block.number(), attestations),
            Err(e) => log::warn!(
                "Failed to fetch attestations for block {}: {:?}",
                block.number(),
                e
            ),
        }

        match storage::read_unstaked_claims_count(&block, api).await {
            Ok(count) => record_claimed_unstake_count(block.number(), count),
            Err(e) => log::warn!(
                "Failed to read unstaked claims count for block {}: {:?}",
                block.number(),
                e
            ),
        }

        let watchlist_balances: Vec<(AccountId32, u128)> =
            futures::future::join_all(self.watchlist.iter().cloned().map(|acct| {
                let block_hash = block.hash();
                async move {
                    (
                        acct.clone(),
                        storage::read_account_free_balance(&acct, block_hash, api)
                            .await
                            .ok()
                            .flatten()
                            .unwrap_or_default(),
                    )
                }
            }))
            .await;

        record_watchlist(block.number(), watchlist_balances);

        // If it's a new session, record rewards and total stake as well
        if has_new_session(&events) {
            match read_active_era(&block, api).await {
                Ok(Some(active)) => {
                    // We have an active era, so let's try to get rewards details
                    let prev_era = active.saturating_sub(1);
                    if let Ok(Some(rewards)) = read_era_rewards(prev_era, &block, api).await {
                        record_era_rewards(prev_era, rewards);
                    }
                    if let Ok(Some(total_staked)) = read_total_staked(prev_era, &block, api).await {
                        record_era_total_stake(prev_era, total_staked);
                    }
                }
                Ok(None) => {
                    // There was no error retriving the era from the chain, but the current era was
                    // None
                    log::warn!("Active Era was None for block {:?}", block.hash());
                }
                Err(e) => {
                    log::error!("Error trying to retrieve active_era {e:?}");
                }
            }
        }
    }
}

/// Error type
#[derive(Debug, Snafu)]
pub enum CanaryError {
    /// ApiConnection error
    #[snafu(display("Failed to connect to Substrate API: {source}"))]
    ApiConnection {
        /// Source error
        source: Box<subxt::Error>,
    },

    /// Error setting up the chain listener
    #[snafu(display("Failed to initialize ChainListener: {source}"))]
    ChainSetup {
        /// Source error
        source: Box<dyn std::error::Error>,
    },

    /// An error originating from the chain listener lib
    #[snafu(display("ChainListenerLibraryError: {source}"))]
    ChainListenerLibError {
        /// source
        source: Box<event_forwarder::block_processing::Error>,
    },
}

impl From<subxt::Error> for CanaryError {
    fn from(err: subxt::Error) -> Self {
        CanaryError::ApiConnection {
            source: Box::new(err),
        }
    }
}

impl From<event_forwarder::block_processing::Error> for CanaryError {
    fn from(err: event_forwarder::block_processing::Error) -> Self {
        CanaryError::ChainListenerLibError {
            source: Box::new(err),
        }
    }
}

type Result<T, E = CanaryError> = std::result::Result<T, E>;
