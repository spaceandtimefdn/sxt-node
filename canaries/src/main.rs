//! Canary prototype
use std::net::SocketAddr;

use axum::routing::get;
use axum::Router;
use clap::Parser;
use event_forwarder::block_processing::fetch_events;
use event_forwarder::chain_listener::{
    Block,
    BlockProcessor,
    ChainListener,
    FinalizedBlockStream,
    API,
};
use lazy_static::lazy_static;
use log::{error, info, warn};
use prometheus::{
    register_gauge_vec,
    register_int_counter_vec,
    Encoder,
    GaugeVec,
    IntCounterVec,
    TextEncoder,
};
use snafu::{ResultExt, Snafu};
use subxt::{OnlineClient, PolkadotConfig};
use sxt_core::sxt_chain_runtime;
use sxt_core::sxt_chain_runtime::api::attestations::events::BlockAttested;
use sxt_core::sxt_chain_runtime::api::indexing::events::{DataSubmitted, QuorumReached};
use sxt_core::sxt_chain_runtime::api::rewards::events::{EraPaid, Payout, PayoutError, SetupError};
use sxt_core::sxt_chain_runtime::api::session::events::NewSession;
use tokio::net::TcpListener;
use url::Url;

lazy_static! {
    /// Prometheus event counter
    pub static ref EVENT_COUNTER: IntCounterVec = register_int_counter_vec!(
        "canary_event_total",
        "Total count of specific events observed in finalized blocks",
        &["type"]
    )
    .unwrap();

    /// Annualized Reward rate tracker
    pub static ref REWARD_RATE: GaugeVec = register_gauge_vec!(
        "canary_era_reward_rate",
        "Annualized staking reward rate per era",
        &["era"]
    ).unwrap();


    /// Total staked tracker
    pub static ref TOTAL_STAKED: GaugeVec = register_gauge_vec!(
        "canary_era_total_staked",
        "Total stake observed for a given era",
        &["era"]
    ).unwrap();

    /// Validator reward tracker
    pub static ref VALIDATOR_REWARD: GaugeVec = register_gauge_vec!(
        "canary_era_validator_reward",
        "Total validator rewards distributed in an era",
        &["era"]
    ).unwrap();

    /// Annaulizer tracker, used for debugging
    pub static ref ANNUALIZER: GaugeVec = register_gauge_vec!(
        "canary_era_annualizer_multiplier",
        "Annualizer multiplier used in APR calculation",
        &["era"]
    ).unwrap();
}

/// Serve prometheus metrics
pub async fn serve_metrics(bind_addr: SocketAddr) -> anyhow::Result<()> {
    let app = Router::new().route("/metrics", get(metrics_handler));

    let listener = TcpListener::bind(bind_addr).await?;
    log::info!(
        "📊 Prometheus metrics server running on http://{}",
        bind_addr
    );

    axum::serve(listener, app).await?;
    Ok(())
}
async fn metrics_handler() -> String {
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}

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
    env_logger::init();
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

#[async_trait::async_trait]
impl BlockProcessor for DummyProcessor {
    async fn process_block(&mut self, _api: &API, block: Block) {
        let block_number = block.number();

        let attested = fetch_events::<BlockAttested>(&block)
            .await
            .unwrap_or_default();

        let submitted = fetch_events::<DataSubmitted>(&block)
            .await
            .unwrap_or_default();

        let quorum = fetch_events::<QuorumReached>(&block)
            .await
            .unwrap_or_default();

        let era_paid = fetch_events::<EraPaid>(&block)
            .await
            .unwrap_or_default()
            .len();
        let payout_error = fetch_events::<PayoutErro>(&block)
            .await
            .unwrap_or_default()
            .len();
        let setup_error = fetch_events::<SetupError>(&block)
            .await
            .unwrap_or_default()
            .len();
        let payout = fetch_events::<Payout>(&block)
            .await
            .unwrap_or_default()
            .len();

        let attested_count = attested.len();
        let submitted_count = submitted.len();
        let quorum_count = quorum.len();

        EVENT_COUNTER
            .with_label_values(&["BlockAttested"])
            .inc_by(attested_count as u64);
        EVENT_COUNTER
            .with_label_values(&["DataSubmitted"])
            .inc_by(submitted_count as u64);
        EVENT_COUNTER
            .with_label_values(&["QuorumReached"])
            .inc_by(quorum_count as u64);
        EVENT_COUNTER
            .with_label_values(&["EraPaid"])
            .inc_by(era_paid_count as u64);
        EVENT_COUNTER
            .with_label_values(&["SetupError"])
            .inc_by(setup_error as u64);
        EVENT_COUNTER
            .with_label_values(&["PayoutError"])
            .inc_by(payout_error as u64);
        EVENT_COUNTER
            .with_label_values(&["BlockAttested"])
            .inc_by(attested_count as u64);
        EVENT_COUNTER
            .with_label_values(&["DataSubmitted"])
            .inc_by(submitted_count as u64);
        EVENT_COUNTER
            .with_label_values(&["Payout"])
            .inc_by(payout as u64);

        let session_events = fetch_events::<NewSession>(&block).await.unwrap_or_default();

        if let Some(event) = session_events.first() {
            let session_index = event.session_index;
            info!("🧭 New session detected: {}", session_index);

            let active_era_query = sxt_chain_runtime::api::storage().staking().active_era();
            let active_era = match _api
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
            let era_reward = match _api
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
            let total_staked = match _api
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
            let annualized_rate = reward_rate * self.annualizer;

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
            ANNUALIZER
                .with_label_values(&[era_label])
                .set(self.annualizer);

            info!(
            "🧱 Block {} | BlockAttested: {}, DataSubmitted: {}, QuorumReached: {} | Era {}: reward={}, staked={}, rate={:.6}, annualizer={}, APR={:.2}%",
                block_number,
                attested_count,
                submitted_count,
                quorum_count,
                prev_era,
                era_reward,
                total_staked,
                reward_rate,
                self.annualizer,
                annualized_rate * 100.0
            );
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
