//! Canary prototype
mod metrics;
use std::net::SocketAddr;

use clap::Parser;
use env_logger::Env;
use event_forwarder::block_processing::{fetch_all_events, filter_events};
use event_forwarder::chain_listener::{
    Block,
    BlockProcessor,
    ChainListener,
    FinalizedBlockStream,
    API,
};
use log::{error, info};
use snafu::{ResultExt, Snafu};
use subxt::events::EventDetails;
use subxt::{OnlineClient, PolkadotConfig};
use sxt_core::sxt_chain_runtime;
use sxt_core::sxt_chain_runtime::api::session::events::NewSession;
use url::Url;

use crate::metrics::*;

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

    /// Multiplier used to annualize per-era reward rates
    #[arg(long, env = "CANARY_ANNUALIZER", default_value_t = 8760.0)]
    pub annualizer: f64,
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
    let api = OnlineClient::<PolkadotConfig>::from_insecure_url(&config.rpc_url)
        .await
        .context(ApiConnectionSnafu)?;

    // Set up the listener
    let stream = FinalizedBlockStream;
    let processor = DummyProcessor {
        annualizer: config.annualizer,
    };

    let listener = ChainListener::new(processor, stream, api)
        .await
        .map_err(|e| CanaryError::ChainSetup { source: e })?;

    listener.run().await;

    Ok(())
}

struct DummyProcessor {
    annualizer: f64,
}
use std::collections::HashMap;
#[async_trait::async_trait]
impl BlockProcessor for DummyProcessor {
    async fn process_block(&mut self, _api: &API, block: Block) {
        let events = fetch_all_events(&block).await.unwrap_or_default();

        // Count all the events in the block by their pallet and variant
        let mut event_count = HashMap::<String, u64>::new();
        for e in events.iter() {
            let label = format!("{:?}.{:?}", e.pallet_name(), e.variant_name());
            match event_count.get(&label) {
                Some(count) => {
                    event_count.insert(label, count + 1);
                }
                None => {
                    event_count.insert(label, 1);
                }
            }
        }

        event_count.iter().for_each(|(label, count)| {
            EVENT_COUNTER.with_label_values(&[label]).inc_by(*count);
        });

        count_reward_stats(&block, &events, _api, self.annualizer).await;

        count_balance_stats(&events);
    }
}

fn count_balance_stats(events: &Vec<EventDetails<PolkadotConfig>>) {
    use sxt_core::sxt_chain_runtime::api::balances::events::{
        Burned,
        Frozen,
        Issued,
        Locked,
        Minted,
        Rescinded,
        Reserved,
        Thawed,
        Transfer,
        Unlocked,
        Unreserved,
    };
    // Unfortunately Prometheus is natively using 64 bit numbers, so the best we can do  for
    // balance handling is saturated_into and then make sure the time frame we look at on the
    // dashboard is using the rate instead of the absolute total value
    events.iter().for_each(|event| {
        if event.pallet_name().contains("Balances") {
            if let Ok(Some(e)) = event.as_event::<Issued>() {
                record_balance("Issued", e.amount);
            } else if let Ok(Some(e)) = event.as_event::<Burned>() {
                record_balance("Burned", e.amount);
            } else if let Ok(Some(e)) = event.as_event::<Unreserved>() {
                record_balance("Unreserved", e.amount);
            } else if let Ok(Some(e)) = event.as_event::<Frozen>() {
                record_balance("Frozen", e.amount);
            } else if let Ok(Some(e)) = event.as_event::<Thawed>() {
                record_balance("Thawed", e.amount);
            } else if let Ok(Some(e)) = event.as_event::<Locked>() {
                record_balance("Locked", e.amount);
            } else if let Ok(Some(e)) = event.as_event::<Minted>() {
                record_balance("Minted", e.amount);
            } else if let Ok(Some(e)) = event.as_event::<Reserved>() {
                record_balance("Reserved", e.amount);
            } else if let Ok(Some(e)) = event.as_event::<Transfer>() {
                record_balance("Transfer", e.amount);
            } else if let Ok(Some(e)) = event.as_event::<Unlocked>() {
                record_balance("Unlocked", e.amount);
            } else if let Ok(Some(e)) = event.as_event::<Rescinded>() {
                record_balance("Rescinded", e.amount);
            }
        }
    });
}

fn record_balance(label: &str, amount: u128) {
    use subxt::ext::sp_runtime::SaturatedConversion;

    BALANCE_COUNTER
        .with_label_values(&[label])
        .inc_by(amount.saturated_into());
}

async fn count_reward_stats(
    block: &Block,
    events: &Vec<EventDetails<PolkadotConfig>>,
    api: &API,
    annualizer: f64,
) {
    let session_events = filter_events::<NewSession>(&events);

    if let Some(event) = session_events.first() {
        let session_index = event.session_index;
        info!("🧭 New session detected: {}", session_index);

        let active_era_query = sxt_chain_runtime::api::storage().staking().active_era();
        let active_era = match api
            .storage()
            .at(block.hash())
            .fetch(&active_era_query)
            .await
        {
            Ok(Some(info)) => info.index,
            Ok(None) => {
                info!("No active era found at block {}", block.number());
                return;
            }
            Err(e) => {
                error!("❌ Failed to fetch active era: {:?}", e);
                return;
            }
        };

        let prev_era = active_era.saturating_sub(1);

        let era_reward_query = sxt_chain_runtime::api::storage()
            .staking()
            .eras_validator_reward(prev_era);
        let era_reward = match api
            .storage()
            .at(block.hash())
            .fetch(&era_reward_query)
            .await
        {
            Ok(Some(val)) => val,
            Ok(None) => {
                info!("No reward data found for era {}", prev_era);
                return;
            }
            Err(e) => {
                error!("❌ Failed to fetch validator reward: {:?}", e);
                return;
            }
        };

        let total_staked_query = sxt_chain_runtime::api::storage()
            .staking()
            .eras_total_stake(prev_era);
        let total_staked = match api
            .storage()
            .at(block.hash())
            .fetch(&total_staked_query)
            .await
        {
            Ok(Some(val)) => val,
            Ok(None) => {
                info!("No total stake found for era {}", prev_era);
                return;
            }
            Err(e) => {
                error!("❌ Failed to fetch total stake: {:?}", e);
                return;
            }
        };

        if total_staked == 0 {
            info!("⚠️ Total stake for era {} is zero, skipping", prev_era);
            return;
        }

        let reward_rate = era_reward as f64 / total_staked as f64;
        let annualized_rate = reward_rate * annualizer;

        let era_label = &prev_era.to_string();

        REWARD_RATE
            .with_label_values(&[era_label])
            .set(annualized_rate * 100.0);

        TOTAL_STAKED
            .with_label_values(&[era_label])
            .set(total_staked as f64);
        VALIDATOR_REWARD
            .with_label_values(&[era_label])
            .set(era_reward as f64);
        ANNUALIZER.with_label_values(&[era_label]).set(annualizer);
    }
}

/// Error type
#[derive(Debug, Snafu)]
pub enum CanaryError {
    /// ApiConnection error
    #[snafu(display("Failed to connect to Substrate API: {source}"))]
    ApiConnection {
        /// Source error
        source: subxt::Error,
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
        source: event_forwarder::block_processing::Error,
    },
}

type Result<T, E = CanaryError> = std::result::Result<T, E>;
