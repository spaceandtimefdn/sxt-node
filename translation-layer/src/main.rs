//! translation layer
use std::sync::Arc;
use std::time::Duration;

use axum::routing::{get, post};
use axum::Router;
use clap::Parser;
use log::info;
use subxt::backend::rpc::reconnecting_rpc_client::{PingConfig, RpcClient};
use subxt::OnlineClient;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Mutex};
use tower_http::cors::{Any, CorsLayer};
use translation_layer::state::{Network, TranslationLayerState};
use translation_layer::tx_progress::TxProgressDb;
use translation_layer::tx_submitter::TxSubmitter;
use translation_layer::{api, signer};
use utoipa::{Modify, OpenApi};
use utoipa_swagger_ui::SwaggerUi;

/// Translation Layer CLI
#[derive(Parser, Debug)]
#[command(
    name = "translation-layer",
    about = "Substrate ↔️ Smart Contract Translation Layer"
)]
struct Cli {
    /// URL of the mainnet Substrate node (e.g. http://localhost:9944)
    #[arg(long, env = "MAINNET_URL", help = "URL of the mainnet Substrate node")]
    mainnet_url: String,

    /// Path to the key file used for signing mainnet transactions (e.g. .mainnet)
    #[arg(
        long,
        env = "MAINNET_KEY",
        help = "Path to the mainnet key file for signing transactions"
    )]
    mainnet_key: String,

    /// URL of the testnet Substrate node (e.g. http://localhost:9944)
    #[arg(long, env = "TESTNET_URL", help = "URL of the testnet Substrate node")]
    testnet_url: String,

    /// Path to the key file used for signing testnet transactions (e.g. .testnet)
    #[arg(
        long,
        env = "TESTNET_KEY",
        help = "Path to the testnet key file for signing transactions"
    )]
    testnet_key: String,

    /// Address and port to bind the HTTP server (default: 127.0.0.1:3000)
    #[arg(
        long,
        env = "BIND_ADDR",
        default_value = "127.0.0.1:3000",
        help = "Address to bind the Axum HTTP server"
    )]
    bind_addr: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logger with fallback to info if RUST_LOG is not set
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();

    info!("🚀 Starting Translation Layer...");

    // Channel for tracking transaction progress
    let (tx, rx) = mpsc::channel(100);
    let tx_db = Arc::new(TxProgressDb::new(rx));
    tokio::spawn({
        let tx_db = tx_db.clone();
        async move {
            info!("🟡 TxProgressDb is running...");
            tx_db.run().await;
        }
    });

    // ──────────────── MAINNET ────────────────
    info!("🔵 Connecting to mainnet: {}", cli.mainnet_url);
    let mainnet_rpc_client = RpcClient::builder()
        .request_timeout(Duration::from_secs(60))
        .connection_timeout(Duration::from_secs(10))
        .enable_ws_ping(PingConfig::new())
        .build(cli.mainnet_url.clone())
        .await?;

    let mainnet_api = OnlineClient::from_rpc_client(mainnet_rpc_client).await?;

    let mainnet_key = signer::load_substrate_key(&cli.mainnet_key).await?;
    let mainnet_submitter = Arc::new(Mutex::new(
        TxSubmitter::new(
            mainnet_api.clone(),
            mainnet_key,
            tx.clone(),
            cli.mainnet_url,
        )
        .await?,
    ));
    info!("🟢 Mainnet TxSubmitter initialized.");

    let mainnet_state = Arc::new(TranslationLayerState {
        network: Network::Mainnet,
        mainnet_submitter: Some(mainnet_submitter),
        testnet_submitter: None,
        tx_db: tx_db.clone(),
        client: Arc::new(mainnet_api),
    });

    // ──────────────── TESTNET ────────────────
    info!("🔵 Connecting to testnet: {}", cli.testnet_url);
    let testnet_rpc_client = RpcClient::builder()
        .request_timeout(Duration::from_secs(60))
        .connection_timeout(Duration::from_secs(10))
        .enable_ws_ping(PingConfig::new())
        .build(cli.testnet_url.clone())
        .await?;

    let testnet_api = OnlineClient::from_rpc_client(testnet_rpc_client).await?;

    let testnet_key = signer::load_substrate_key(&cli.testnet_key).await?;
    let testnet_submitter = Arc::new(Mutex::new(
        TxSubmitter::new(
            testnet_api.clone(),
            testnet_key,
            tx.clone(),
            cli.testnet_url,
        )
        .await?,
    ));
    info!("🟢 Testnet TxSubmitter initialized.");

    let testnet_state = Arc::new(TranslationLayerState {
        network: Network::Testnet,
        mainnet_submitter: None,
        testnet_submitter: Some(testnet_submitter),
        tx_db: tx_db.clone(),
        client: Arc::new(testnet_api),
    });

    // ──────────────── ROUTING ────────────────
    info!("🔧 Setting up routing...");
    let swagger = SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi());

    let common_routes = |state: Arc<TranslationLayerState>| {
        Router::new()
            .route(
                "/add_smartcontract",
                post(api::smartcontracts::add_smartcontract),
            )
            .route(
                "/remove_smartcontract",
                post(api::smartcontracts::remove_smartcontract),
            )
            .route(
                "/get_smartcontract",
                get(api::smartcontracts::get_smartcontract),
            )
            .route("/create_table", post(api::tables::create_table))
            .route(
                "/get_extrinsic_status_in_block",
                get(api::extrinsics::get_extrinsic_status_in_block),
            )
            .route(
                "/get_extrinsic_status",
                get(api::extrinsics::get_extrinsic_status),
            )
            .route("/drop_table", post(api::tables::drop_table))
            .with_state(state)
    };

    let app = Router::new()
        .nest("/api/mainnet", common_routes(mainnet_state))
        .nest("/api/testnet", common_routes(testnet_state))
        .merge(swagger)
        .layer(CorsLayer::new().allow_origin(Any));

    info!("🟢 Routes set up successfully.");
    info!("🔵 Binding to {}", cli.bind_addr);

    let listener = TcpListener::bind(&cli.bind_addr).await?;
    info!("🚀 Server running on http://{}", cli.bind_addr);

    axum::serve(listener, app).await?;
    Ok(())
}

#[derive(OpenApi)]
#[openapi(paths(
    api::smartcontracts::add_smartcontract,
    api::smartcontracts::remove_smartcontract,
    api::smartcontracts::get_smartcontract,
    api::smartcontracts::get_smartcontracts,
    api::tables::create_table,
    api::tables::drop_table,
    api::extrinsics::get_extrinsic_status_in_block,
    api::extrinsics::get_extrinsic_status,
), modifiers(&AddRoutePrefixes))]
struct ApiDoc;

struct AddRoutePrefixes;

impl Modify for AddRoutePrefixes {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let original_paths = std::mem::take(&mut openapi.paths.paths);

        for (path, path_item) in original_paths {
            openapi
                .paths
                .paths
                .insert(format!("/api/mainnet{}", path), path_item.clone());

            openapi
                .paths
                .paths
                .insert(format!("/api/testnet{}", path), path_item);
        }
    }
}
