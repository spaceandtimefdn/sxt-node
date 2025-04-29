use attestation_tree::{CommitmentMapPrefixFoliate, LocksStakingPrefixFoliate, PrefixFoliate};
use frame_support::traits::StorageInstance;
use futures::{TryFutureExt, TryStreamExt};
use pallet_system_contracts::_GeneratedPrefixForStorageStakingContract;
use snafu::{ResultExt, Snafu};
use subxt::backend::{BackendExt, StorageResponse};
use subxt::{OnlineClient, PolkadotConfig};
use sxt_runtime::Runtime;
use tokio::try_join;

/// Errors that may occur while fetching blockchain data.
#[derive(Debug, Snafu)]
pub enum FetchError {
    /// Error encountered while iterating over locks storage in the blockchain.
    ///
    /// This occurs when accessing and processing locks stored on-chain.
    #[snafu(display("Failed to iterate over locks: {source}"))]
    LocksStorageIteration {
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

    /// Error encountered while fetching staking contract info from the blockchain.
    #[snafu(display("Failed to fetch staking contract info: {source}"))]
    StakingContractFetch {
        /// The underlying error from Substrate's storage fetch.
        source: subxt::Error,
    },

    /// Error encountered if fetching the staking contract succeeded, but it does not exist.
    #[snafu(display("Staking contract info does not exist"))]
    NoStakingContract,
}

/// Fetches commitments, locks data, and staking contract info for a given block.
///
/// # Arguments
/// * `api` - Reference to the Substrate API client.
/// * `block_hash` - The hash of the block to fetch data from.
///
/// # Returns
/// A `Result` containing a tuple of 3 items:
/// - list of commitment storage key-value pairs as raw bytes
/// - list of locks storage key-value pairs as raw bytes
/// - staking contract info as raw bytes
pub async fn commitments_and_locks_and_staking_contract_info(
    api: &OnlineClient<PolkadotConfig>,
    block_hash: subxt::utils::H256,
) -> Result<(Vec<(Vec<u8>, Vec<u8>)>, Vec<(Vec<u8>, Vec<u8>)>, Vec<u8>), FetchError> {
    try_join!(
        async {
            api
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
        .context(CommitmentStorageIterationSnafu)
        },
        async {
            api
        .backend()
        .storage_fetch_descendant_values(
            <<LocksStakingPrefixFoliate<Runtime> as PrefixFoliate>::StorageInstance as StorageInstance>::prefix_hash()
                .to_vec(),
            block_hash,
        )
        .and_then(|stream| {
            stream
                .map_ok(|StorageResponse { key, value }| (key, value))
                .try_collect::<Vec<_>>()
        })
        .await
        .context(LocksStorageIterationSnafu)
        },
        async {
            api.backend()
                .storage_fetch_value(
                    _GeneratedPrefixForStorageStakingContract::<Runtime>::prefix_hash().to_vec(),
                    block_hash,
                )
                .await
                .context(StakingContractFetchSnafu)
                .and_then(|maybe_staking_contract| {
                    maybe_staking_contract.ok_or(FetchError::NoStakingContract)
                })
        }
    )
}
