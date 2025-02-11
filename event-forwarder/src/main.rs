//! todo
use std::str::FromStr;
use std::sync::Arc;

use alloy::hex::FromHexError;
use alloy::network::EthereumWallet;
use alloy::primitives::Address;
use alloy::providers::ProviderBuilder;
use alloy::signers::local::PrivateKeySigner;
use alloy::transports::http::reqwest::Url;
use clap::{Parser, Subcommand};
use event_forwarder::chain_listener::{ChainListener, FinalizedBlockStream};
use event_forwarder::event_forwarder::{EventForwarderProcessor, ProviderInstance};
use event_forwarder::kitchen_sink::KitchenSinkProcessor;
use hex::FromHex;
use k256::ecdsa::SigningKey;
use log::info;
use sha3::digest::generic_array::GenericArray;
use snafu::{ResultExt, Snafu};
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use url::ParseError;

#[derive(Debug, Snafu)]
enum EventForwarderError {
    #[snafu(display("Failed to parse URL: {}", source))]
    UrlParse { source: ParseError },

    #[snafu(display("Failed to read Ethereum key from file '{}': {}", path, source))]
    KeyFileRead {
        path: String,
        source: std::io::Error,
    },

    #[snafu(display("Failed to parse Ethereum key as hex: {}", source))]
    KeyParse { source: hex::FromHexError },

    #[snafu(display("Invalid contract address format: {}", source))]
    AddressParse { source: FromHexError },

    #[snafu(display("Blockchain processing error: {}", source))]
    BlockchainProcessing { source: Box<dyn std::error::Error> },
}

/// Type alias for returning results with `CustomError`
type Result<T, E = EventForwarderError> = std::result::Result<T, E>;

/// CLI arguments parser using `clap` derive syntax
#[derive(Parser, Debug)]
#[command(
    name = "Blockchain Processor",
    version = "1.0",
    author = "Your Name <your.email@example.com>",
    about = "Listens to blockchain events and processes them"
)]
struct Cli {
    /// The RPC URL of the Ethereum node
    #[arg(short, long, default_value = "ws://127.0.0.1:9944")]
    rpc_url: String,

    /// The contract address
    #[arg(
        short,
        long,
        default_value = "0xf93fc53262fdb57302577Ab880150F626aE164ff"
    )]
    contract_address: String,

    /// Path to the Ethereum key file
    #[arg(short, long, default_value = ".eth")]
    eth_key_path: String,

    /// Subcommands (e.g., integration-test)
    #[command(subcommand)]
    command: Option<Commands>,
}

/// Defines the available subcommands
#[derive(Subcommand, Debug)]
enum Commands {
    /// Runs an integration test for blockchain event processing
    IntegrationTest,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize env_logger
    env_logger::init();

    // Parse command-line arguments
    let args = Cli::parse();

    // If a subcommand is provided, execute it
    if let Some(command) = args.command {
        match command {
            Commands::IntegrationTest => {
                return run_integration_test().await;
            }
        }
    }

    // Default behavior: Run normal blockchain processor
    let rpc_url = Url::from_str(&args.rpc_url).context(UrlParseSnafu)?;
    let ethereum_signer = load_ethereum_key(&args.eth_key_path).await?;
    let signer = PrivateKeySigner::from_signing_key(ethereum_signer);
    let wallet = EthereumWallet::from(signer.clone());

    // Set up the HTTP provider
    let provider: Arc<ProviderInstance> =
        Arc::new(ProviderBuilder::new().wallet(wallet).on_http(rpc_url));

    let contract_address = Address::from_str(&args.contract_address).context(AddressParseSnafu)?;

    info!("Starting blockchain processor...");
    let processor = EventForwarderProcessor::new(provider.clone(), contract_address);

    // Use finalized block processing
    let chain_listener =
        ChainListener::<EventForwarderProcessor, FinalizedBlockStream>::new(processor)
            .await
            .context(BlockchainProcessingSnafu)?;

    chain_listener.run().await;
    Ok(())
}

/// Runs the integration test
async fn run_integration_test() -> Result<()> {
    let rpc_url =
        Url::from_str("https://eth-sepolia.g.alchemy.com/v2/rkAXO6gJwI3eR9jVZeCcY5ejjpVxGkw8")
            .context(UrlParseSnafu)?;

    let ethereum_signer = load_ethereum_key(".eth").await?;
    let signer = PrivateKeySigner::from_signing_key(ethereum_signer);
    let wallet = EthereumWallet::from(signer.clone());

    // Set up the HTTP provider
    let provider: Arc<ProviderInstance> =
        Arc::new(ProviderBuilder::new().wallet(wallet).on_http(rpc_url));

    let address = Address::from_str("0xf93fc53262fdb57302577Ab880150F626aE164ff")
        .context(AddressParseSnafu)?;

    info!("Starting integration test...");
    let processor = KitchenSinkProcessor::from_existing_deployment(provider.clone(), address)
        .await
        .context(BlockchainProcessingSnafu)?;

    // Use finalized block processing
    let chain_listener =
        ChainListener::<KitchenSinkProcessor, FinalizedBlockStream>::new(processor)
            .await
            .context(BlockchainProcessingSnafu)?;

    chain_listener.run().await;
    Ok(())
}

async fn load_ethereum_key(path: &str) -> Result<SigningKey> {
    let mut file = File::open(path).await.context(KeyFileReadSnafu {
        path: path.to_string(),
    })?;
    let mut hex_string = String::new();
    file.read_to_string(&mut hex_string)
        .await
        .context(KeyFileReadSnafu {
            path: path.to_string(),
        })?;

    let key_bytes = Vec::from_hex(hex_string.trim()).context(KeyParseSnafu)?;
    let key_array = GenericArray::from_slice(&key_bytes);
    Ok(SigningKey::from_bytes(key_array).unwrap()) // `unwrap` is safe since key_array is always valid length
}
