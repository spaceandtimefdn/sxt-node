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
use alloy::hex::FromHexError;
use alloy::network::EthereumWallet;
use alloy::primitives::{Address, FixedBytes, Uint};
use alloy::providers::ProviderBuilder;
use alloy::signers::local::PrivateKeySigner;
use alloy::transports::http::reqwest::Url;
use clap::{Parser, Subcommand};
use codec::{Decode, Encode};
use event_forwarder::chain_listener::{ChainListener, IncrementingBlockStream};
use event_forwarder::event_forwarder::{EventForwarderInstance, ProviderInstance};
use event_forwarder::event_forwarder_contract::EventForwarder;
use event_forwarder::kitchen_sink::KitchenSinkProcessor;
use hex::FromHex;
use itertools::Itertools;
use k256::ecdsa::SigningKey;
use log::{error, info};
use sha3::digest::generic_array::GenericArray;
use snafu::{ResultExt, Snafu};
use sp_core::crypto::AccountId32;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use subxt::utils::H256;
use subxt::{OnlineClient, PolkadotConfig};
use subxt_signer::sr25519::Keypair;
use sxt_core::sxt_chain_runtime;
use sxt_core::sxt_chain_runtime::api::runtime_types::sxt_core::attestation::{
    Attestation, EthereumSignature,
};
use sxt_core::system_tables::ClaimedUnstake;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;
use url::ParseError;
use watcher::attestation::fetch::commitments_and_locks_and_staking_contract_info_and_claimed_unstakes;

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

    #[snafu(transparent)]
    SubxtError { source: subxt::Error },
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

#[derive(Debug, Snafu)]
enum BlockProcessingError {
    #[snafu(transparent)]
    FetchError {
        source: watcher::attestation::fetch::FetchError,
    },
    #[snafu(transparent)]
    CodecError { source: codec::Error },
    #[snafu(transparent)]
    SubxtError { source: subxt::Error },
}

async fn attestations_per_root(
    config: &Config,
    block_number: u32,
) -> Result<HashMap<Vec<u8>, Vec<EthereumSignature>>, subxt::Error> {
    let attestations_storage_address = sxt_chain_runtime::api::storage()
        .attestations()
        .attestations(block_number);

    let maybe_attestations = config
        .api
        .storage()
        .at_latest()
        .await?
        .fetch(&attestations_storage_address)
        .await?;

    let attestations = maybe_attestations.map(|a| a.0).unwrap_or_default();

    let result: HashMap<Vec<u8>, Vec<EthereumSignature>> = attestations
        .into_iter()
        .map(
            |Attestation::EthereumAttestation {
                 state_root,
                 signature,
                 ..
             }| (state_root.0, signature),
        )
        .into_group_map();
    Ok(result)
}

async fn attempt_fulfill_unstake(
    contract: &EventForwarderInstance,
    claimed_unstake: ClaimedUnstake<AccountId32, u32, u128>,
    claim_attestations: &[EthereumSignature],
) {
    let staker = Address::from_slice(&<[u8; 32]>::from(claimed_unstake.staker)[12..32]);
    let claimed_amount = Uint::from(claimed_unstake.claimed_amount);
    let claim_block_number = claimed_unstake.claim_block_number.into();
    let proof = vec![];
    let (r, s, v) = claim_attestations
        .iter()
        .map(|e| (FixedBytes::from(e.r), FixedBytes::from(e.s), e.v))
        .multiunzip();
    match contract
        .sxtFulfillUnstake(staker, claimed_amount, claim_block_number, proof, r, s, v)
        .send()
        .await
    {
        Ok(tx) => info!("sxtFulfillUnstake tx sent: {}", tx.tx_hash()),
        Err(e) => error!("Failed to send transaction: {}", e),
    }
}

async fn process_block(
    config: &Config,
    contract: &EventForwarderInstance,
    block_hash: H256,
    block_number: u32,
) -> Result<(), BlockProcessingError> {
    let (_, _, staking_contract_info, claimed_unstakes) =
        commitments_and_locks_and_staking_contract_info_and_claimed_unstakes(
            &config.api,
            block_hash,
        )
        .await?;
    let claimed_unstakes: Vec<_> = Decode::decode(&mut claimed_unstakes.encode().as_slice())?;
    let contract_info = Decode::decode(&mut staking_contract_info.as_slice())?;

    let attestations_per_root = attestations_per_root(config, block_number).await?;

    for claimed_unstake in claimed_unstakes {
        let claimed_unstake_attestation_leaf =
            sxt_core::attestation::claimed_unstake_attestation_leaf::<u32>(
                &claimed_unstake,
                &contract_info,
            );

        let maybe_claim_attestations = attestations_per_root.get(&claimed_unstake_attestation_leaf);

        info!(
            "Found {} attestation(s) for claim ({}, {}, {}).",
            maybe_claim_attestations.map(|m| m.len()).unwrap_or(0),
            claimed_unstake.staker,
            claimed_unstake.claim_block_number,
            claimed_unstake.claimed_amount
        );

        if let Some(claim_attestations) = maybe_claim_attestations {
            attempt_fulfill_unstake(contract, claimed_unstake, &claim_attestations);
        }
    }
    Ok(())
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

    let contract = EventForwarder::new(config.contract_address, config.provider.clone());

    let mut block_sub = config.api.blocks().subscribe_best().await?;

    while let Some(block) = block_sub.next().await {
        match block {
            Ok(block) => {
                let block_hash = block.hash();
                let block_number = block.number();
                info!("Processing block: {} ({:?})", block_number, block_hash);

                match process_block(&config, &contract, block_hash, block_number).await {
                    Ok(_) => {
                        info!(
                            "Successfuly processed block: {} ({:?})",
                            block_number, block_hash
                        );
                    }
                    Err(e) => {
                        log::error!("Error processing block: {}", e);
                    }
                }
            }
            Err(e) => {
                log::error!("Error receiving block: {}", e);
            }
        }
    }
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
