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
use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;
use std::sync::Arc;

use alloy::hex::FromHexError;
use alloy::network::{Ethereum, EthereumWallet};
use alloy::primitives::{Address, FixedBytes, Uint};
use alloy::providers::fillers::{
    BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller, WalletFiller,
};
use alloy::providers::{Identity, ProviderBuilder, RootProvider, WsConnect};
use alloy::signers::local::PrivateKeySigner;
use alloy::transports::TransportError;
use clap::Parser;
use codec::{Decode, Encode};
use hex::FromHex;
use itertools::Itertools;
use k256::ecdsa::SigningKey;
use log::{error, info};
use sha3::digest::generic_array::GenericArray;
use snafu::{ResultExt, Snafu};
use sp_core::crypto::AccountId32;
use sp_core::keccak_256;
use subxt::utils::H256;
use subxt::{OnlineClient, PolkadotConfig};
use sxt_core::sxt_chain_runtime;
use sxt_core::sxt_chain_runtime::api::runtime_types::sxt_core::attestation::{
    Attestation, EthereumSignature,
};
use sxt_core::system_tables::ClaimedUnstake;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use watcher::attestation::fetch::commitments_and_staking_contract_info_and_claimed_unstakes;

#[allow(clippy::too_many_arguments, missing_docs)]
mod event_forwarder_contract {
    use alloy::sol;
    sol!(
        /// event forwarder contract
        #[sol(rpc)]
        EventForwarder,
        "artifacts/EventForwarder.json"
    );
}
use event_forwarder_contract::*;

type ProviderInstance = FillProvider<
    JoinFill<
        JoinFill<
            Identity,
            JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
        >,
        WalletFiller<EthereumWallet>,
    >,
    RootProvider,
    Ethereum,
>;

#[allow(clippy::missing_docs_in_private_items)]
#[derive(Debug, Snafu)]
enum EventForwarderError {
    #[snafu(transparent)]
    AlloyTransportError { source: TransportError },

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

    #[snafu(transparent)]
    SubxtError { source: subxt::Error },
}

/// CLI arguments parser using `clap` derive syntax
#[derive(Parser, Debug)]
#[command(
    name = "Space and Time Event Forwarder",
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

    /// The substrate rpc url
    #[arg(long, default_value = "ws://127.0.0.1:9944")]
    substrate_rpc_url: String,
}

#[derive(Debug, Snafu)]
enum BlockProcessingError {
    #[snafu(transparent)]
    Fetch {
        source: watcher::attestation::fetch::FetchError,
    },
    #[snafu(transparent)]
    Codec { source: codec::Error },
    #[snafu(transparent)]
    Subxt { source: subxt::Error },
}

async fn attestations_per_root(
    config: &Config,
    block_number: u32,
) -> Result<HashMap<Vec<u8>, BTreeMap<String, EthereumSignature>>, subxt::Error> {
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

    let result: HashMap<Vec<u8>, BTreeMap<String, EthereumSignature>> = attestations
        .into_iter()
        .fold(HashMap::new(), |mut acc, attestation| {
            let Attestation::EthereumAttestation {
                state_root,
                address20,
                signature,
                ..
            } = attestation;
            acc.entry(state_root.0)
                .or_default()
                .insert(hex::encode(address20.0), signature);
            acc
        });
    Ok(result)
}

async fn attempt_fulfill_unstake<P: alloy::providers::Provider>(
    contract: &EventForwarder::EventForwarderInstance<(), P>,
    claimed_unstake: ClaimedUnstake<AccountId32, u32, u128>,
    claim_attestations: impl IntoIterator<Item = &EthereumSignature>,
) {
    let staker = Address::from_slice(&<[u8; 32]>::from(claimed_unstake.staker)[12..32]);
    let claimed_amount = Uint::from(claimed_unstake.claimed_amount);
    let claim_block_number = claimed_unstake.claim_block_number.into();
    let proof = vec![];
    let (r, s, v) = claim_attestations
        .into_iter()
        .map(|e| (FixedBytes::from(e.r), FixedBytes::from(e.s), e.v + 27))
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

async fn process_block<P: alloy::providers::Provider>(
    config: &Config,
    contract: &EventForwarder::EventForwarderInstance<(), P>,
    block_hash: H256,
    block_number: u32,
) -> Result<(), BlockProcessingError> {
    let (_, staking_contract_info, claimed_unstakes) =
        commitments_and_staking_contract_info_and_claimed_unstakes(&config.api, block_hash).await?;
    let claimed_unstakes: Vec<_> = Decode::decode(&mut claimed_unstakes.encode().as_slice())?;
    let contract_info = Decode::decode(&mut staking_contract_info.as_slice())?;

    let attestations_per_root = attestations_per_root(config, block_number).await?;

    for claimed_unstake in claimed_unstakes {
        let claimed_unstake_attestation_leaf =
            sxt_core::attestation::claimed_unstake_attestation_leaf::<u32>(
                &claimed_unstake,
                &contract_info,
            );

        let claimed_unstake_root_hash =
            keccak_256(&keccak_256(&claimed_unstake_attestation_leaf)).to_vec();

        let maybe_claim_attestations = attestations_per_root.get(&claimed_unstake_root_hash);

        info!(
            "Found {} attestation(s) for claim ({}, {}, {}).",
            maybe_claim_attestations.map(|m| m.len()).unwrap_or(0),
            claimed_unstake.staker,
            claimed_unstake.claim_block_number,
            claimed_unstake.claimed_amount,
        );

        if let Some(claim_attestations) = maybe_claim_attestations {
            attempt_fulfill_unstake(contract, claimed_unstake, claim_attestations.values()).await;
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), EventForwarderError> {
    env_logger::init();

    // Parse CLI arguments
    let args = Cli::parse();

    // Run the normal blockchain processor
    let config = setup_config(
        &args.rpc_url,
        &args.eth_key_path,
        &args.contract_address,
        &args.substrate_rpc_url,
    )
    .await?;

    let contract = EventForwarder::new(config.contract_address, config.provider.clone());

    let mut block_sub = config.api.blocks().subscribe_finalized().await?;

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
) -> Result<Config, EventForwarderError> {
    let rpc_url = WsConnect::new(rpc_url);
    let ethereum_signer = load_ethereum_key(eth_key_path).await?;
    let signer = PrivateKeySigner::from_signing_key(ethereum_signer);
    let wallet = EthereumWallet::from(signer.clone());

    let provider: Arc<ProviderInstance> =
        Arc::new(ProviderBuilder::new().wallet(wallet).on_ws(rpc_url).await?);

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

async fn load_ethereum_key(path: &str) -> Result<SigningKey, EventForwarderError> {
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
