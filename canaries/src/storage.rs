use anyhow::Result;
use event_forwarder::chain_listener::{Block, API};
use subxt::utils::AccountId32;
use subxt::{Config, PolkadotConfig};
use sxt_core::sxt_chain_runtime::api::runtime_apis::system_tables_api::SystemTablesApi;
use sxt_core::sxt_chain_runtime::{self};

/// Reads the active era, if one is present. Returns Some(u32) if an era is found, None if there is
/// no era, and an error if we were not able to read it.
pub(crate) async fn read_active_era(block: &Block, api: &API) -> Result<Option<u32>> {
    let active_era_query = sxt_chain_runtime::api::storage().staking().active_era();

    if let Some(active_era) = api
        .storage()
        .at(block.hash())
        .fetch(&active_era_query)
        .await?
    {
        Ok(Some(active_era.index))
    } else {
        Ok(None)
    }
}

/// Reads the total validator rewards for the specified era.
pub(crate) async fn read_era_rewards(era: u32, block: &Block, api: &API) -> Result<Option<u128>> {
    let era_reward_query = sxt_chain_runtime::api::storage()
        .staking()
        .eras_validator_reward(era);
    if let Some(era_reward) = api
        .storage()
        .at(block.hash())
        .fetch(&era_reward_query)
        .await?
    {
        Ok(Some(era_reward))
    } else {
        Ok(None)
    }
}

/// Reads the total staked amount for the specified era.
pub(crate) async fn read_total_staked(era: u32, block: &Block, api: &API) -> Result<Option<u128>> {
    let total_staked_query = sxt_chain_runtime::api::storage()
        .staking()
        .eras_total_stake(era);
    if let Some(total_staked) = api
        .storage()
        .at(block.hash())
        .fetch(&total_staked_query)
        .await?
    {
        Ok(Some(total_staked))
    } else {
        Ok(None)
    }
}

/// Reads the free balance of an account at a given block.
/// Returns Some(balance) if a balance exists, otherwise returns None.
pub(crate) async fn read_account_free_balance(
    account: &AccountId32,
    block_hash: <PolkadotConfig as Config>::Hash,
    api: &API,
) -> Result<Option<u128>> {
    let free_balance_query = sxt_chain_runtime::api::storage().system().account(account);
    if let Some(info) = api
        .storage()
        .at(block_hash)
        .fetch(&free_balance_query)
        .await?
    {
        Ok(Some(info.data.free))
    } else {
        Ok(None)
    }
}

/// Reads the count of claimed unstakes for the given block.
pub(crate) async fn read_unstaked_claims_count(block: &Block, api: &API) -> Result<u64> {
    let claims = api
        .runtime_api()
        .at(block.hash())
        .call(SystemTablesApi.claimed_unstakes())
        .await?;

    Ok(claims.len() as u64)
}
