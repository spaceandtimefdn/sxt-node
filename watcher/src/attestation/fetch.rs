use attestation_tree::{AccountPrefixFoliate, CommitmentMapPrefixFoliate, PrefixFoliate};
use frame_support::traits::StorageInstance;
use futures::{TryFutureExt, TryStreamExt};
use snafu::{ResultExt, Snafu};
use subxt::backend::StorageResponse;
use subxt::{OnlineClient, PolkadotConfig};
use sxt_runtime::Runtime;

/// Errors that may occur while fetching blockchain data.
#[derive(Debug, Snafu)]
pub enum FetchError {
    /// Error encountered while iterating over account storage in the blockchain.
    ///
    /// This occurs when accessing and processing account balances stored on-chain.
    #[snafu(display("Failed to iterate over accounts: {source}"))]
    AccountStorageIteration {
        /// The underlying error from Substrate's storage iteration.
        source: subxt::Error,
    },

    /// Error encountered while iterating over commitment storage in the blockchain.
    ///
    /// This happens when accessing and processing commitment records stored on-chain.
    #[snafu(display("Failed to iterate over commitments: {source}"))]
    CommitmentStorageIteration {
        /// The underlying error from Substrate's storage iteration.
        source: subxt::Error,
    },
}

/// Fetches both commitments and account data for a given block.
///
/// # Arguments
/// * `api` - Reference to the Substrate API client.
/// * `block_hash` - The hash of the block to fetch data from.
///
/// # Returns
/// A `Result` containing a tuple of two lists:
/// - commitment storage key-value pairs as raw bytes
/// - account storage key-value pairs as raw bytes
pub async fn commitments_and_accounts(
    api: &OnlineClient<PolkadotConfig>,
    block_hash: subxt::utils::H256,
) -> Result<(Vec<(Vec<u8>, Vec<u8>)>, Vec<(Vec<u8>, Vec<u8>)>), FetchError> {
    let commitment_prefix_iter = api
        .backend()
        .storage_fetch_descendant_values(
            <<CommitmentMapPrefixFoliate::<Runtime> as PrefixFoliate>::StorageInstance as StorageInstance>::prefix_hash().to_vec(),
            block_hash,
        )
        .and_then(|stream| {
            stream
                .map_ok(|StorageResponse { key, value }| (key, value))
                .try_collect::<Vec<_>>()
        })
        .await
        .context(CommitmentStorageIterationSnafu)?;

    let account_prefix_iter = api
        .backend()
        .storage_fetch_descendant_values(
            <<AccountPrefixFoliate<Runtime> as PrefixFoliate>::StorageInstance as StorageInstance>::prefix_hash()
                .to_vec(),
            block_hash,
        )
        .and_then(|stream| {
            stream
                .map_ok(|StorageResponse { key, value }| (key, value))
                .try_collect::<Vec<_>>()
        })
        .await
        .context(AccountStorageIterationSnafu)?;

    Ok((commitment_prefix_iter, account_prefix_iter))
}
