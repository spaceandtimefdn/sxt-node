//! # Blockchain Event Listener
//!
//! The `chain_listener` module provides an efficient framework for **subscribing to blockchain blocks**,
//! **streaming them in real-time**, and **processing events** using custom handlers.
//! It abstracts both **finalized** and **non-finalized** block processing,
//! allowing developers to seamlessly integrate **Substrate-based** blockchain data into their applications.
//!
//! ## Features
//! - **Real-Time Block Processing**: Subscribe to finalized or non-finalized blocks using Substrate’s API.
//! - **Custom Event Processing**: Implements a `BlockProcessor` trait for handling block events.
//! - **Incremental Block Fetching**: Fetches blocks one by one and processes them in sequence.
//! - **Fault Tolerance**: Includes retry logic and logging for network errors.
//!
//! ## Overview
//! The module consists of three key components:
//!
//! - [`BlockProcessor`]: A trait that defines how blocks should be processed.
//! - [`BlockStreamProvider`]: A trait that provides different strategies for streaming blockchain blocks.
//! - [`ChainListener`]: A core struct that listens to blocks and triggers processing logic.
//!
//! ## Block Streaming Strategies
//! The module provides multiple implementations for fetching blocks:
//!
//! - [`FinalizedBlockStream`]: Streams only finalized blocks.
//! - [`NonFinalizedBlockStream`]: Streams non-finalized blocks (best chain).
//! - [`IncrementingBlockStream`]: Fetches blocks sequentially based on an external trigger signal.

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use async_stream::stream;
use async_trait::async_trait;
use jsonrpsee::core::client::ClientT;
use jsonrpsee::ws_client::WsClientBuilder;
use log::{error, info};
use reqwest::Client;
use serde_json::json;
use subxt::backend::StreamOf;
use subxt::utils::H256;
use subxt::{OnlineClient, PolkadotConfig};
use tokio::sync::mpsc::Receiver;
use tokio::sync::Mutex;
use tokio::time::sleep;

/// The `API` type represents a connected Substrate client that interacts with the blockchain.
pub type API = OnlineClient<PolkadotConfig>;

/// Represents a **block structure** retrieved from the blockchain, used in processing.
pub type Block = subxt::blocks::Block<PolkadotConfig, API>;

/// Defines behavior for processing blocks (finalized or non-finalized).
///
/// Implement this trait to handle **custom business logic** when a block is received.
/// Common use cases include **attestation validation**, **state updates**, and **event tracking**.
#[async_trait]
pub trait BlockProcessor {
    /// Called when a new block is received.
    ///
    /// # Arguments
    /// - `api`: A reference to the blockchain API for querying additional data.
    /// - `block`: The received block to be processed.
    async fn process_block(&mut self, api: &API, block: Block);
}

/// Defines a trait that provides a **stream of new blocks**.
///
/// Implementations of this trait determine **how blocks are streamed**,
/// whether via a subscription (real-time) or sequentially based on external triggers.
#[async_trait]
pub trait BlockStreamProvider {
    /// Returns a **stream of blockchain blocks**.
    ///
    /// # Arguments
    /// - `api`: The Substrate API client.
    ///
    /// # Returns
    /// - A stream of `Block` results, handling errors in block retrieval.
    async fn block_stream(
        self,
        api: &API,
    ) -> Result<subxt::backend::StreamOf<Result<Block, subxt::Error>>, subxt::Error>;
}

/// A **generic block listener** that subscribes to blockchain blocks and processes them.
///
/// This struct listens to new blocks using a given [`BlockStreamProvider`] and delegates
/// block processing to a provided [`BlockProcessor`].
///
/// ## Features
/// - Listens for new blocks (finalized or non-finalized).
/// - Processes each block using the specified `BlockProcessor`.
/// - Handles subscription failures gracefully with logging.
pub struct ChainListener<T, S>
where
    T: BlockProcessor + Send + Sync,
    S: BlockStreamProvider + Send + Sync,
{
    api: API,
    processor: T,
    stream: S,
    _marker: std::marker::PhantomData<S>,
}

impl<T, S> ChainListener<T, S>
where
    T: BlockProcessor + Send + Sync,
    S: BlockStreamProvider + Send + Sync,
{
    /// Creates a new `ChainListener`.
    ///
    /// # Arguments
    /// - `processor`: The block processor that will handle new blocks.
    /// - `stream`: The block stream provider that determines how blocks are fetched.
    /// - `api`: The blockchain API client.
    ///
    /// # Returns
    /// - A new `ChainListener` instance.
    pub async fn new(
        processor: T,
        stream: S,
        api: OnlineClient<PolkadotConfig>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            api,
            processor,
            stream,
            _marker: std::marker::PhantomData,
        })
    }

    /// Starts listening for blocks and processes them using the `BlockProcessor`.
    ///
    /// This function **subscribes to the blockchain**, listens for **new blocks**, and calls
    /// `process_block()` for each one. If the subscription fails, it logs an error.
    pub async fn run(mut self) {
        info!("Starting blockchain processor...");

        let mut block_sub = match self.stream.block_stream(&self.api).await {
            Ok(subscription) => subscription,
            Err(e) => {
                error!("Failed to subscribe to blocks: {}", e);
                return;
            }
        };

        while let Some(block) = block_sub.next().await {
            match block {
                Ok(block) => {
                    let block_hash = block.hash();
                    let block_number = block.number();
                    info!("Processing block: {} ({:?})", block_number, block_hash);

                    self.processor.process_block(&self.api, block).await;
                }
                Err(e) => {
                    error!("Error receiving block: {}", e);
                }
            }
        }
    }
}

/// Provides a **stream of finalized blocks**.
///
/// This implementation of [`BlockStreamProvider`] subscribes to finalized blocks
/// from the Substrate network.
pub struct FinalizedBlockStream;

#[async_trait]
impl BlockStreamProvider for FinalizedBlockStream {
    async fn block_stream(
        self,
        api: &API,
    ) -> Result<subxt::backend::StreamOf<Result<Block, subxt::Error>>, subxt::Error> {
        api.blocks().subscribe_finalized().await
    }
}

/// Provides a **stream of non-finalized blocks** (best chain).
///
/// This is useful when working with **live data** that may be reorged.
pub struct NonFinalizedBlockStream;

#[async_trait]
impl BlockStreamProvider for NonFinalizedBlockStream {
    async fn block_stream(
        self,
        api: &API,
    ) -> Result<subxt::backend::StreamOf<Result<Block, subxt::Error>>, subxt::Error> {
        api.blocks().subscribe_best().await
    }
}

/// A **block stream that increments manually** based on an external signal.
///
/// This stream fetches blocks **one at a time**, waiting for a **success signal** before proceeding.
/// It's useful for **sequential processing workflows** that require **explicit control**.
pub struct IncrementingBlockStream {
    /// The first block to begin processing
    start_block: u32,
    /// Channel that indicates when it is time to increment to the next block
    receiver: Arc<Mutex<Receiver<bool>>>,
    /// Address where the block stream can fetch the initial nonce from
    substrate_rpc_url: String,
}

impl IncrementingBlockStream {
    /// Creates a new `IncrementingBlockStream` starting from a given block number.
    pub fn new(start_block: u32, receiver: Receiver<bool>, substrate_rpc_url: String) -> Self {
        Self {
            start_block,
            receiver: Arc::new(Mutex::new(receiver)),
            substrate_rpc_url,
        }
    }

    /// Fetches the block hash for a given block number using JSON-RPC.
    pub async fn get_block_hash(&self, block_number: u32) -> anyhow::Result<Option<String>> {
        let client = WsClientBuilder::default()
            .build(&self.substrate_rpc_url)
            .await?;

        let result: Option<String> = client.request("chain_getBlockHash", [block_number]).await?;

        Ok(result)
    }

    async fn fetch_block(
        &self,
        api: &API,
        block_number: u32,
    ) -> Result<Option<Block>, subxt::Error> {
        match self.get_block_hash(block_number).await {
            Ok(Some(block_hash)) => {
                let block_hash = H256::from_str(&block_hash)
                    .map_err(|_| subxt::Error::Other("Invalid block hash format".into()))?;

                api.blocks().at(block_hash).await.map(Some)
            }
            Ok(None) => Ok(None),
            Err(e) => Err(subxt::Error::Other(format!(
                "Failed to fetch block hash: {}",
                e
            ))),
        }
    }
}

#[async_trait]
impl BlockStreamProvider for IncrementingBlockStream {
    async fn block_stream(
        self,
        api: &API,
    ) -> Result<StreamOf<Result<Block, subxt::Error>>, subxt::Error> {
        let mut current_block = self.start_block;
        let api = api.clone();
        let receiver = Arc::clone(&self.receiver); // Clone the Arc to share receiver

        let stream = stream! {
            // Send the first block before entering the loop
            match self.fetch_block(&api, current_block).await {
                Ok(Some(block)) => {
                    info!("Processing first block: {}", current_block);
                    yield Ok(block);
                }
                Ok(None) => {
                    error!("No hash found for first block {}", current_block);
                }
                Err(e) => {
                    error!("Failed to fetch first block {}: {}", current_block, e);
                }
            }

            loop {
                let mut rx = receiver.lock().await;  // Lock the receiver before receiving
                match rx.recv().await {
                    Some(true) => {
                        current_block += 1; // Increment only after a success signal

                        match self.fetch_block(&api, current_block).await {
                            Ok(Some(block)) => {
                                info!("Processing block: {}", current_block);
                                yield Ok(block);
                            }
                            Ok(None) => error!("No hash found for block {}", current_block),
                            Err(e) => error!("Failed to fetch block {}: {}", current_block, e),
                        }
                    }
                    Some(false) => error!("Received failure signal. Retrying..."),
                    None => {
                        error!("Channel closed. Stopping block stream.");
                        break;
                    }
                }

                sleep(Duration::from_secs(1)).await; // Avoid spamming requests
            }
        };

        Ok(StreamOf::new(Box::pin(stream)))
    }
}

fn convert_ws_to_https(url: &str) -> String {
    url.replacen("ws://", "http://", 1)
        .replacen("wss://", "https://", 1)
}
