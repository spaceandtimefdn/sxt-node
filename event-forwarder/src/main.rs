//! This binary runs a blockchain event processor that listens for events on a Substrate-based blockchain,
//! processes attestations, and forwards relevant data to an Ethereum smart contract.
//!
//! ## Features
//! - Listens for finalized blockchain blocks.
//! - Processes attestations and staking/unbonding events.
//! - Computes Merkle tree proofs for validation.
//! - Forwards staking attestations to an Ethereum contract.
//! - Provides an integration test mode to verify full event processing.
//!
//! ## Usage
//! ```sh
//! cargo run -- --rpc-url ws://127.0.0.1:9944 --contract-address 0xf93fc53262fdb57302577Ab880150F626aE164ff --eth-key-path .eth --substrate-key-path .sxt
//! ```
//!
//! To run the integration test mode:
//! ```sh
//! cargo run -- integration-test
//! ```
use std::str::FromStr;
use std::sync::Arc;

use alloy::hex::FromHexError;
use alloy::network::EthereumWallet;
use alloy::primitives::Address;
use alloy::providers::ProviderBuilder;
use alloy::signers::local::PrivateKeySigner;
use alloy::transports::http::reqwest::Url;
use clap::{Parser, Subcommand};
use event_forwarder::chain_listener::{ChainListener, IncrementingBlockStream};
use event_forwarder::event_forwarder::{EventForwarderProcessor, ProviderInstance};
use event_forwarder::kitchen_sink::KitchenSinkProcessor;
use hex::FromHex;
use k256::ecdsa::SigningKey;
use log::info;
use sha3::digest::generic_array::GenericArray;
use snafu::{ResultExt, Snafu};
use subxt::{OnlineClient, PolkadotConfig};
use subxt_signer::sr25519::Keypair;
use sxt_core::sxt_chain_runtime;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;
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

    #[snafu(display("Invalid key length: expected 32 bytes, got {}", length))]
    InvalidKeyLength { length: usize },

    #[snafu(display("Failed to create keypair from secret key"))]
    KeypairCreationError,

    #[snafu(display("Error fetching last forwarded block: {source}"))]
    LastForwardedBlockError { source: subxt::Error },

    #[snafu(display("Error fetching initial nonce: {source}"))]
    FetchInitialNonceError { source: subxt::Error },
}

/// Type alias for returning results with `CustomError`
type Result<T, E = EventForwarderError> = std::result::Result<T, E>;

/// CLI arguments parser using `clap` derive syntax
#[derive(Parser, Debug)]
#[command(
    name = "Space and Time Event Forwarder",
    version = "1.0",
    author = "zach.frederick@spaceandtime.io",
    about = "Forwards events from the SxT chain back to Ethereum for support of staking and ZKPay"
)]
struct Cli {
    /// The RPC URL of the Ethereum node
    #[arg(long, default_value = "ws://127.0.0.1:9944")]
    rpc_url: String,

    /// The contract address
    #[arg(long, default_value = "0xd27Da90dfaabE287B572919A6f0aeEBc79a2Ed7e")]
    contract_address: String,

    /// Path to the Ethereum key file
    #[arg(long, default_value = ".eth")]
    eth_key_path: String,

    /// The file path to the Substrate SR25519 private key.
    ///
    /// This key is used to submit transactions to the blockchain.
    #[arg(long, default_value = ".substrate")]
    substrate_key_path: String,

    /// Subcommands (e.g., integration-test)
    #[command(subcommand)]
    command: Option<Commands>,

    /// The substrate rpc url
    #[arg(long, default_value = "ws://127.0.0.1:9944")]
    substrate_rpc_url: String,
}

/// Defines the available subcommands
#[derive(Subcommand, Debug)]
enum Commands {
    /// Runs an integration test for blockchain event processing
    IntegrationTest,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    // Parse CLI arguments
    let args = Cli::parse();

    // If a subcommand is provided, execute it
    if let Some(Commands::IntegrationTest) = args.command {
        return run_integration_test().await;
    }

    // Run the normal blockchain processor
    let config = setup_config(
        &args.rpc_url,
        &args.eth_key_path,
        &args.contract_address,
        &args.substrate_rpc_url,
    )
    .await?;
    let keypair = load_substrate_key(&args.substrate_key_path).await?;
    let initial_nonce = fetch_initial_nonce(&config.api, &keypair).await?;

    let (tx, rx) = mpsc::channel(1);
    let start_block = fetch_start_block(&config.api).await?;
    let stream = IncrementingBlockStream::new(start_block, rx, args.substrate_rpc_url);

    let processor = EventForwarderProcessor::new(
        config.provider.clone(),
        config.contract_address,
        keypair,
        Some(tx),
        initial_nonce.into(),
    );

    let chain_listener = ChainListener::new(processor, stream, config.api)
        .await
        .context(BlockchainProcessingSnafu)?;

    chain_listener.run().await;
    Ok(())
}

/// Runs the integration test
async fn run_integration_test() -> Result<()> {
    let config = setup_config(
        "https://eth-sepolia.g.alchemy.com/v2/rkAXO6gJwI3eR9jVZeCcY5ejjpVxGkw8",
        ".eth",
        "0xf93fc53262fdb57302577Ab880150F626aE164ff",
        "ws://127.0.0.1:9944",
    )
    .await?;

    let keypair = load_substrate_key(".substrate").await?;
    let initial_nonce = fetch_initial_nonce(&config.api, &keypair).await?;

    let (tx, rx) = mpsc::channel(1);
    let start_block = fetch_start_block(&config.api).await?;
    let stream = IncrementingBlockStream::new(start_block, rx, "http://127.0.0.1:9944".into());

    info!("Starting integration test...");
    let processor = KitchenSinkProcessor::from_existing_deployment(
        config.provider.clone(),
        config.contract_address,
        Some(tx),
        keypair,
        initial_nonce.into(),
    )
    .await
    .context(BlockchainProcessingSnafu)?;

    let chain_listener = ChainListener::new(processor, stream, config.api)
        .await
        .context(BlockchainProcessingSnafu)?;

    chain_listener.run().await;
    Ok(())
}

/// Holds shared configuration for the blockchain processor and integration test
struct Config {
    provider: Arc<ProviderInstance>,
    contract_address: Address,
    api: OnlineClient<PolkadotConfig>,
}

/// Initializes common configuration used in both main and integration test
async fn setup_config(
    rpc_url: &str,
    eth_key_path: &str,
    contract_address: &str,
    substrate_rpc_url: &str,
) -> Result<Config> {
    let rpc_url = Url::from_str(rpc_url).context(UrlParseSnafu)?;
    let ethereum_signer = load_ethereum_key(eth_key_path).await?;
    let signer = PrivateKeySigner::from_signing_key(ethereum_signer);
    let wallet = EthereumWallet::from(signer.clone());

    let provider: Arc<ProviderInstance> =
        Arc::new(ProviderBuilder::new().wallet(wallet).on_http(rpc_url));

    let contract_address = Address::from_str(contract_address.trim()).context(AddressParseSnafu)?;

    let api = OnlineClient::<PolkadotConfig>::from_insecure_url(substrate_rpc_url)
        .await
        .map_err(|e| EventForwarderError::BlockchainProcessing {
            source: Box::new(e),
        })?;

    Ok(Config {
        provider,
        contract_address,
        api,
    })
}

/// Fetches the initial nonce for a given keypair
async fn fetch_initial_nonce(api: &OnlineClient<PolkadotConfig>, keypair: &Keypair) -> Result<u32> {
    let nonce_query = sxt_chain_runtime::api::storage()
        .system()
        .account(keypair.public_key().to_account_id());

    let nonce = api
        .storage()
        .at_latest()
        .await
        .context(FetchInitialNonceSnafu)?
        .fetch(&nonce_query)
        .await
        .context(FetchInitialNonceSnafu)?;

    if let Some(nonce) = nonce {
        return Ok(nonce.nonce);
    }

    Ok(0)
}

/// Fetches the start block based on the last forwarded block in the chain
async fn fetch_start_block(api: &OnlineClient<PolkadotConfig>) -> Result<u32> {
    let last_forwarded_block_query = sxt_chain_runtime::api::storage()
        .attestations()
        .last_forwarded_block();

    let last_forwarded_block = api
        .storage()
        .at_latest()
        .await
        .context(LastForwardedBlockSnafu)?
        .fetch(&last_forwarded_block_query)
        .await
        .context(LastForwardedBlockSnafu)?
        .unwrap_or(0);

    Ok(if last_forwarded_block == 0 {
        0
    } else {
        last_forwarded_block + 1
    })
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

async fn load_substrate_key(file_path: &str) -> Result<Keypair> {
    let mut file = File::open(file_path).await.context(KeyFileReadSnafu {
        path: file_path.to_string(),
    })?;

    let mut hex_string = String::new();
    file.read_to_string(&mut hex_string)
        .await
        .context(KeyFileReadSnafu {
            path: file_path.to_string(),
        })?;

    let key_bytes = Vec::from_hex(hex_string.trim()).context(KeyParseSnafu)?;

    if key_bytes.len() != 32 {
        return Err(EventForwarderError::InvalidKeyLength {
            length: key_bytes.len(),
        });
    }

    let key_bytes: [u8; 32] =
        key_bytes
            .clone()
            .try_into()
            .map_err(|_| EventForwarderError::InvalidKeyLength {
                length: key_bytes.len(),
            })?;

    Keypair::from_secret_key(key_bytes).map_err(|_| EventForwarderError::KeypairCreationError)
}
