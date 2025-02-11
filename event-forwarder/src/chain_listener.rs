use async_trait::async_trait;
use log::{error, info};
use subxt::{OnlineClient, PolkadotConfig};

/// API type
pub type API = OnlineClient<PolkadotConfig>;

/// Block type
pub type Block = subxt::blocks::Block<PolkadotConfig, API>;

/// Defines behavior for processing blocks (finalized or non-finalized).
#[async_trait]
pub trait BlockProcessor {
    /// Called when a new block is received.
    async fn process_block(&self, api: &API, block: Block);
}

/// Defines a trait that provides a block stream (finalized or non-finalized).
#[async_trait]
pub trait BlockStreamProvider {
    /// Returns a stream of new blocks.
    async fn block_stream(
        api: &API,
    ) -> Result<subxt::backend::StreamOf<Result<Block, subxt::Error>>, subxt::Error>;
}

/// A generic processor that listens for blocks and processes them.
pub struct ChainListener<T, S>
where
    T: BlockProcessor + Send + Sync,
    S: BlockStreamProvider + Send + Sync,
{
    api: API,
    processor: T,
    _marker: std::marker::PhantomData<S>,
}

impl<T, S> ChainListener<T, S>
where
    T: BlockProcessor + Send + Sync,
    S: BlockStreamProvider + Send + Sync,
{
    /// Creates a new `BlockchainProcessor`.
    pub async fn new(processor: T) -> Result<Self, Box<dyn std::error::Error>> {
        let api = OnlineClient::<PolkadotConfig>::new().await?;
        Ok(Self {
            api,
            processor,
            _marker: std::marker::PhantomData,
        })
    }

    /// Starts listening for blocks and processes them.
    pub async fn run(&self) {
        info!("Starting blockchain processor...");

        let mut block_sub = match S::block_stream(&self.api).await {
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

/// Provides a stream of finalized blocks.
pub struct FinalizedBlockStream;

#[async_trait]
impl BlockStreamProvider for FinalizedBlockStream {
    async fn block_stream(
        api: &API,
    ) -> Result<subxt::backend::StreamOf<Result<Block, subxt::Error>>, subxt::Error> {
        api.blocks().subscribe_finalized().await
    }
}

/// Provides a stream of non-finalized blocks.
pub struct NonFinalizedBlockStream;

#[async_trait]
impl BlockStreamProvider for NonFinalizedBlockStream {
    async fn block_stream(
        api: &API,
    ) -> Result<subxt::backend::StreamOf<Result<Block, subxt::Error>>, subxt::Error> {
        api.blocks().subscribe_best().await
    }
}
