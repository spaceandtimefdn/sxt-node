//! This module provides functionality to process finalized blocks and events in a Substrate-based blockchain.
//! It includes utilities for spawning tasks, creating clients, and handling events.

use core::ops::Sub;
use std::sync::Arc;
use std::time::Duration;

use alloy::network::{Ethereum, EthereumWallet};
use alloy::primitives::{address, bytes, Address, Uint, B256, U256};
use alloy::providers::fillers::{
    BlobGasFiller,
    ChainIdFiller,
    FillProvider,
    GasFiller,
    JoinFill,
    NonceFiller,
    WalletFiller,
};
use alloy::providers::{ProviderBuilder, RootProvider, WsConnect};
use alloy::pubsub::PubSubFrontend;
use alloy::signers::local::PrivateKeySigner;
use alloy::{contract, sol};
use arrow::datatypes::ToByteSlice;
use frame_support::__private::log;
use k256::elliptic_curve::rand_core::block;
use sc_client_api::{Backend, BlockchainEvents, Finalizer, StorageProvider};
use serde_json::json;
use snafu::prelude::*;
use snafu::ResultExt;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_core::traits::SpawnEssentialNamed;
use sp_core::Encode;
use subxt::error::RpcError;
use subxt::ext::futures::StreamExt;
use subxt::ext::subxt_core::constants::address;
use subxt::utils::{AccountId32, H256};
use subxt::{OnlineClient, PolkadotConfig};
use EventForwarder::EventForwarderInstance;

use crate::sxt_chain_runtime;
use crate::sxt_chain_runtime::api::attestations::events::BlockAttested;
use crate::sxt_chain_runtime::api::indexing::events::QuorumReached;
use crate::sxt_chain_runtime::api::staking::events::Unbonded;

/// Enum representing errors that can occur during the multiplexer process.
#[derive(Debug, Snafu)]
enum MultiplexerError {
    /// Error when fetching events for a block.
    #[snafu(display("Failed to fetch events for block: {source}"))]
    EventFetch { source: subxt::Error },

    /// Error when decoding an event.
    #[snafu(display("Failed to decode event: {details}"))]
    EventDecode { details: String },

    /// Error when creating a Subxt client.
    #[snafu(display("Failed to create Subxt client: {source}"))]
    ClientCreation { source: subxt::Error },

    #[snafu(display("RPC Error: {source}"))]
    RPCError { source: RpcError },

    #[snafu(display("MissingEventError: {details}"))]
    MissingEvent { details: String },

    /// Error when serializing JSON (for RPC calls).
    #[snafu(display("JSON Serialization Error: {source}"))]
    JsonSerialization { source: serde_json::Error },

    #[snafu(display("Subxt Error: {source}"))]
    SubxtError { source: subxt::Error },

    /// Error when interacting with the smart contract.
    #[snafu(display("Contract Call Error: {source}"))]
    ContractCall { source: contract::Error },
}

/// Spawns the multiplexer task using the provided Spawn handle.
///
/// This function is responsible for spawning an asynchronous task that processes
/// finalized blocks and their associated events.
///
/// # Parameters
/// - `name`: A name for the task.
/// - `spawner`: An implementation of [`SpawnEssentialNamed`] to spawn the task.
/// - `client`: An Arc-wrapped client for accessing blockchain data.
pub fn spawn_multiplexer<Client, Block, BE>(
    name: &'static str,
    spawner: &impl SpawnEssentialNamed,
    client: Arc<Client>,
    key: String,
    rpc: String,
) where
    Client: BlockchainEvents<Block>
        + HeaderBackend<Block>
        + ProvideRuntimeApi<Block>
        + StorageProvider<Block, BE>
        + Finalizer<Block, BE>
        + 'static,
    BE: Backend<Block>,
    Block: sp_runtime::traits::Block,
{
    spawner.spawn_essential_blocking(
        name,
        Some("multiplexer"),
        Box::pin(async move {
            run(client, key, rpc).await;
        }),
    );
}

/// Main event loop for processing finalized blocks and their associated events.
///
/// This function continuously processes finalized blocks streamed from the blockchain.
/// Errors are logged, and the function never returns.
///
/// # Parameters
/// - `chain_client`: A client for accessing blockchain data.
async fn run<Client, Block, BE>(chain_client: Arc<Client>, key: String, rpc: String)
where
    Client: BlockchainEvents<Block>
        + HeaderBackend<Block>
        + StorageProvider<Block, BE>
        + Finalizer<Block, BE>,
    BE: Backend<Block>,
    Block: sp_runtime::traits::Block,
{
    let api = match create_subxt_client().await {
        Ok(client) => client,
        Err(e) => {
            log::error!("Failed to create Subxt client: {:?}", e);
            return;
        }
    };

    let signer: PrivateKeySigner = key.parse().expect("should parse private key");
    let wallet = EthereumWallet::from(signer);

    let ws = WsConnect::new(rpc);
    let provider = match ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(wallet)
        .on_ws(ws)
        .await
    {
        Ok(p) => p,
        Err(e) => {
            log::error!("Failed to create ethereum client: {:?}", e);
            return;
        }
    };

    log::info!("Successfully made ethereum provider!");

    let contract_address = address!("fADACEFff0C2054eDF3F9f6F91341748854B8273");
    let event_forwarder = Arc::new(EventForwarder::new(contract_address, provider));

    let mut stream = chain_client.finality_notification_stream();

    while let Some(finalized_block) = stream.next().await {
        if let Ok(block) = api
            .blocks()
            .at(H256::from_slice(finalized_block.hash.as_ref()))
            .await
        {
            if let Err(e) = process_block_events(&block, event_forwarder.clone(), api.clone()).await
            {
                log::error!("Error processing block events: {:?}", e);
            }
        }
    }
}

/// Processes events for a single Subxt block.
///
/// Fetches events from the block and decodes them. Any errors during
/// event fetching or decoding are logged.
///
/// # Parameters
/// - `block`: A Subxt block whose events are to be processed.
///    Processes events for a single Subxt block.
///
/// Fetches events from the block and decodes them. Any errors during
/// event fetching or decoding are logged.
///
/// # Parameters
/// - `block`: A Subxt block whose events are to be processed.
async fn process_block_events(
    block: &subxt::blocks::Block<PolkadotConfig, OnlineClient<PolkadotConfig>>,
    event_forwarder: EventForwarderImpl,
    client: OnlineClient<PolkadotConfig>,
) -> Result<Vec<BlockAttested>, MultiplexerError> {
    let mut attestations = Vec::new();

    // Fetch all events in the block
    let events = block.events().await.context(EventFetchSnafu).map_err(|e| {
        log::error!("Failed to fetch events for block: {:?}", e);
        e
    })?;

    // Iterate through the events
    for event_result in events.iter() {
        match event_result {
            Ok(event_details) => {
                // Attempt to decode `BlockAttested` events and add them to the list
                if let Ok(Some(attestation)) =
                    decode_attestation(Ok(event_details), event_forwarder.clone()).await
                {
                    attestations.push(attestation);
                }
            }
            Err(e) => {
                log::error!("Failed to get event details: {:?}", e);
            }
        }
    }

    if attestations.is_empty() {
        return Ok(attestations);
    }

    let attestation0 = attestations.first().unwrap();
    let params =
        serde_json::to_vec(&json!([attestation0.block_number])).context(JsonSerializationSnafu)?;

    let attestation_hash = client
        .backend()
        .call("chain_getBlockHash", Some(&params), block.hash())
        .await
        .context(SubxtSnafu)?;

    let attestation_hash = H256::from_slice(&attestation_hash);

    let attested_block = client
        .blocks()
        .at(attestation_hash)
        .await
        .context(SubxtSnafu)?;

    // TODO iterate over the events in this block, look for a staking unbonded events
    let events = attested_block
        .events()
        .await
        .context(EventFetchSnafu)
        .map_err(|e| {
            log::error!("Failed to fetch events for block: {:?}", e);
            e
        })?;

    let mut unbondings = Vec::new();

    // Iterate through the events
    for event_result in events.iter() {
        match event_result {
            Ok(event_details) => {
                // Attempt to decode `BlockAttested` events and add them to the list
                if let Ok(Some(attestation)) =
                    decode_unbonding(Ok(event_details), event_forwarder.clone()).await
                {
                    unbondings.push(attestation);
                }
            }
            Err(e) => {
                log::error!("Failed to get event details: {:?}", e);
            }
        }
    }

    for unbonding in unbondings {
        let account_id = unbonding.stash;

        match fetch_read_proof(&client, attestation_hash, &account_id).await {
            Err(e) => log::error!("❌ Failed to fetch read proof: {:?}", e),
            Ok(proof) => {
                let amount = unbonding.amount;

                let address = Address::from_slice(account_id.0.as_ref());
                let amount: Uint<248, 4> = Uint::from(amount);

                let proof_bytes = convert_vec_to_bytes32(proof);

                // iterate over attestations pull out r,s,v values and put them into r,s,v types that can be used with alloy
                // Extract r, s, v values from EthereumSignature
                let (r_values, s_values, v_values) =
                    unzip3(attestations.iter().filter_map(|attestation| {
                        if let BlockAttested {
                            attestation: EthereumAttestation { signature, .. },
                            ..
                        } = attestation
                        {
                            Some((
                                B256::from_slice(&signature.r), // Convert r to B256
                                B256::from_slice(&signature.s), // Convert s to B256
                                signature.v,                    // v is already a u8
                            ))
                        } else {
                            None
                        }
                    }));

                let result = event_forwarder
                    .processUnstake(address, amount, proof_bytes, r_values, s_values, v_values)
                    .send()
                    .await
                    .context(ContractCallSnafu)?;
            }
        }
    }

    Ok(attestations)
}

use std::convert::TryInto;

use crate::sxt_chain_runtime::api::runtime_types::sxt_core::attestation::Attestation::EthereumAttestation;

fn unzip3<T, U, V>(iter: impl IntoIterator<Item = (T, U, V)>) -> (Vec<T>, Vec<U>, Vec<V>) {
    let mut vec_t = Vec::new();
    let mut vec_u = Vec::new();
    let mut vec_v = Vec::new();

    for (t, u, v) in iter {
        vec_t.push(t);
        vec_u.push(u);
        vec_v.push(v);
    }

    (vec_t, vec_u, vec_v)
}

fn convert_vec_to_bytes32(proof: Vec<u8>) -> Vec<B256> {
    let mut proof_chunks = vec![];

    // Pad `proof` to a multiple of 32 bytes
    let mut padded_proof = proof.clone();
    while padded_proof.len() % 32 != 0 {
        padded_proof.push(0);
    }

    // Convert into 32-byte chunks
    for chunk in padded_proof.chunks(32) {
        let b256_chunk: B256 = B256::from_slice(chunk);
        proof_chunks.push(b256_chunk);
    }

    proof_chunks
}

/// Fetches a read proof for an account balance at a given block hash.
async fn fetch_read_proof(
    client: &OnlineClient<PolkadotConfig>,
    block_hash: H256,
    account_id: &AccountId32,
) -> Result<Vec<u8>, MultiplexerError> {
    // Construct the correct storage key manually
    let storage_key = sxt_chain_runtime::api::storage()
        .system()
        .account(account_id)
        .to_root_bytes();

    let params = serde_json::to_vec(&json!([vec![hex::encode(storage_key)]]))
        .context(JsonSerializationSnafu)?;

    // Call `state_getReadProof` RPC to get the Merkle proof
    let proof: Vec<u8> = client
        .backend()
        .call("state_getReadProof", Some(&params), block_hash)
        .await
        .context(SubxtSnafu)?;

    Ok(proof)
}

/// Handles a single event by decoding and processing it.
///
/// Decodes the event and performs custom logic based on the decoded event type.
///
/// # Parameters
/// - `event_details`: Details of the event to be processed.
///     Handles a single event by decoding and processing it.
///
/// Decodes the event and performs custom logic based on the decoded event type.
///
/// # Parameters
/// - `event_details`: Details of the event to be processed.
/// - `event_forwarder`: Forwarder for additional processing.
///
/// # Returns
/// Returns `Ok(Some(BlockAttested))` if the event is successfully decoded, otherwise `Ok(None)`.
async fn decode_attestation(
    event_details: Result<subxt::events::EventDetails<PolkadotConfig>, subxt::Error>,
    _event_forwarder: EventForwarderImpl,
) -> Result<Option<BlockAttested>, MultiplexerError> {
    let event = event_details.context(EventFetchSnafu)?;

    let pallet = event.pallet_name();
    let variant = event.variant_name();

    if pallet == "Attestations" && variant == "BlockAttested" {
        // Attempt to decode the `BlockAttested` events
        if let Ok(decoded_event) = event.as_event::<BlockAttested>() {
            // Return the decoded event
            return Ok(Some(decoded_event.unwrap()));
        } else {
            log::warn!("Failed to decode Attestations::BlockAttested event");
        }
    }

    // Return None if the event is not of interest or decoding fails
    Ok(None)
}

async fn decode_unbonding(
    event_details: Result<subxt::events::EventDetails<PolkadotConfig>, subxt::Error>,
    _event_forwarder: EventForwarderImpl,
) -> Result<Option<Unbonded>, MultiplexerError> {
    let event = event_details.context(EventFetchSnafu)?;

    let pallet = event.pallet_name();
    let variant = event.variant_name();

    if pallet == "Staking" && variant == "Unbonded" {
        // Attempt to decode the `BlockAttested` events
        if let Ok(decoded_event) = event.as_event::<Unbonded>() {
            // Return the decoded event
            return Ok(Some(decoded_event.unwrap()));
        } else {
            log::warn!("Failed to decode Attestations::BlockAttested event");
        }
    }

    // Return None if the event is not of interest or decoding fails
    Ok(None)
}

/// Decodes and processes an event of a specific type.
///
/// This helper function abstracts event decoding and logging.
///
/// # Type Parameters
/// - `T`: The event type to decode.
///
/// # Parameters
/// - `event`: The event details to decode.
///
/// # Errors
/// Returns a `MultiplexerError` if decoding fails or the event is missing.
fn decode_event<T>(
    event: &subxt::events::EventDetails<PolkadotConfig>,
) -> Result<T, MultiplexerError>
where
    T: std::fmt::Debug + subxt::events::StaticEvent,
{
    let decoded_event = event
        .as_event::<T>()
        .map_err(|e| MultiplexerError::EventDecode {
            details: format!("{:?}", e),
        })?
        .ok_or_else(|| MultiplexerError::MissingEvent {
            details: format!(
                "Expected event of type {}, but it was missing",
                std::any::type_name::<T>()
            ),
        })?;

    log::info!("Decoded event: {:?}", decoded_event);
    Ok(decoded_event)
}

/// Creates a Subxt client for listening to blocks and events.
///
/// Constructs a client with custom WebSocket parameters to handle larger requests and
/// responses, along with connection and request timeouts.
///
/// # Returns
/// A result containing the created [`OnlineClient`] or an error.
async fn create_subxt_client() -> Result<OnlineClient<PolkadotConfig>, MultiplexerError> {
    let local_node_rpc = String::from("ws://127.0.0.1:9944");

    OnlineClient::<PolkadotConfig>::from_insecure_url(local_node_rpc)
        .await
        .map_err(|e| MultiplexerError::ClientCreation {
            source: e, // Wrap the source error
        })
}

// Generate an EventForwarder using the contract abi pulled from etherscan
sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    EventForwarder,
    "sepolia_event_forwarder_abi.json"
);

/// Convenience type for interacting with a contract instance with recommended fillers and a wallet
pub type EventForwarderImpl = Arc<
    EventForwarderInstance<
        PubSubFrontend,
        FillProvider<
            JoinFill<
                JoinFill<
                    alloy::providers::Identity,
                    JoinFill<
                        GasFiller,
                        JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>,
                    >,
                >,
                WalletFiller<EthereumWallet>,
            >,
            RootProvider<PubSubFrontend>,
            PubSubFrontend,
            Ethereum,
        >,
    >,
>;
