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
use log::info;
use prometheus::{register_int_counter_vec, Encoder, IntCounterVec, TextEncoder};
use snafu::{ResultExt, Snafu};
use subxt::{OnlineClient, PolkadotConfig};
use sxt_core::sxt_chain_runtime::api::attestations::events::BlockAttested;
use sxt_core::sxt_chain_runtime::api::indexing::events::{DataSubmitted, QuorumReached};
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
    let api = OnlineClient::<PolkadotConfig>::from_url(&config.rpc_url)
        .await
        .context(ApiConnectionSnafu)?;

    // Set up the listener
    let stream = FinalizedBlockStream;
    let processor = DummyProcessor;

    let listener = ChainListener::new(processor, stream, api)
        .await
        .map_err(|e| CanaryError::ChainSetup { source: e })?;

    listener.run().await;

    Ok(())
}

struct DummyProcessor;

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

        // Log the result
        info!(
            "🧱 Block {} | BlockAttested: {}, DataSubmitted: {}, QuorumReached: {}",
            block_number, attested_count, submitted_count, quorum_count
        );
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
