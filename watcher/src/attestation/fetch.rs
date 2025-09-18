use attestation_tree::{CommitmentMapPrefixFoliate, PrefixFoliate};
use frame_support::traits::StorageInstance;
use futures::{TryFutureExt, TryStreamExt};
use pallet_system_contracts::_GeneratedPrefixForStorageStakingContract;
use snafu::{ResultExt, Snafu};
use subxt::backend::{BackendExt, StorageResponse};
use subxt::utils::AccountId32;
use subxt::{OnlineClient, PolkadotConfig};
use sxt_core::sxt_chain_runtime::api::runtime_apis::system_tables_api::SystemTablesApi;
use sxt_core::sxt_chain_runtime::api::runtime_types::sxt_core::system_tables::ClaimedUnstake;
use sxt_runtime::Runtime;
use tokio::try_join;

/// Errors that may occur while fetching blockchain data.
#[derive(Debug, Snafu)]
pub enum FetchError {
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

    /// Error encountered while fetching claimed unstake info from the blockchain.
    #[snafu(display("Failed to fetch claimed unstakes: {source}"))]
    ClaimedUnstakesFetch {
        /// The underlying error from Substrate's storage fetch.
        source: subxt::Error,
    },

    /// Error encountered if fetching the staking contract succeeded, but it does not exist.
    #[snafu(display("Staking contract info does not exist"))]
    NoStakingContract,
}

/// Fetches commitments, staking contract info, and claimed unstakes for a given block.
pub async fn commitments_and_staking_contract_info_and_claimed_unstakes(
    api: &OnlineClient<PolkadotConfig>,
    block_hash: subxt::utils::H256,
) -> Result<
    (
        // commitments keys and values
        Vec<(Vec<u8>, Vec<u8>)>,
        // staking contract info
        Vec<u8>,
        // claimed unstakes
        Vec<ClaimedUnstake<AccountId32, u32, u128>>,
    ),
    FetchError,
> {
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
        },
        async {
            claimed_unstakes(api, block_hash)
                .await
                .context(ClaimedUnstakesFetchSnafu)
        }
    )
}

/// Fetches the claimed unstakes storage using a runtime api.
pub async fn claimed_unstakes(
    api: &OnlineClient<PolkadotConfig>,
    block_hash: subxt::utils::H256,
) -> Result<Vec<ClaimedUnstake<AccountId32, u32, u128>>, subxt::Error> {
    api.runtime_api()
        .at(block_hash)
        .call(SystemTablesApi.claimed_unstakes())
        .await
}
