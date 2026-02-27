//! This module holds types and logic related to refunds of indexers who participate in a quorum.
//! It includes the logic associated with generating, checking, and interacting with treasury
//! accounts tied to tables.
extern crate alloc;
use alloc::vec::Vec;

use native_api::NativeApi;
use polkadot_sdk::frame_support::dispatch::DispatchResult;
use polkadot_sdk::frame_support::ensure;
use polkadot_sdk::frame_system::RawOrigin;
use polkadot_sdk::sp_runtime::traits::StaticLookup;
use polkadot_sdk::sp_runtime::{DispatchError, SaturatedConversion};
use polkadot_sdk::{frame_system, pallet_balances};
use sxt_core::tables::TableIdentifier;

use crate::pallet::Config;
use crate::{BatchId, DataQuorum};

/// Refund a single indexer who submitted a late batch
pub fn refund_late_submission<T, I>(
    who: &T::AccountId,
    table: &TableIdentifier,
    refund: u128,
) -> DispatchResult
where
    T: Config<I>,
    I: NativeApi,
{
    if let Some(treasury) = sxt_core::utils::account_id_from_table_id::<T>(table) {
        let free_balance: u128 =
            polkadot_sdk::pallet_balances::Pallet::<T>::free_balance(&treasury).saturated_into();
        ensure!(
            free_balance > refund,
            crate::Error::<T, I>::InsufficientTableFunds
        );
        payout_indexer::<T>(&treasury, who, refund)
    } else {
        Err(crate::Error::<T, I>::InvalidTable.into())
    }
}

/// Refund all quorum participants from the table's treasury. By default everyone gets a full
/// refund. In the future agreements may receive a bonus.
#[allow(clippy::type_complexity)]
pub fn refund_quorum_participants<T, I>(
    quorum: &DataQuorum<T::AccountId, T::Hash>,
    base_refund: u128,
) -> Result<Vec<(T::AccountId, DispatchResult)>, crate::Error<T, I>>
where
    T: Config<I>,
    I: NativeApi,
{
    // Calculate rewards
    let reward_percent = 0;
    // The base refund plus the reward percent
    let reward_amount = base_refund.saturating_add(
        base_refund
            .saturating_mul(reward_percent)
            .saturating_div(100),
    );

    // Make sure we have enough in the treasury to payout for this quorum
    let num_agreements = quorum.agreements.len() as u128;
    let num_dissents = quorum.dissents.len() as u128;

    let total_needed = num_agreements
        .saturating_mul(reward_amount)
        .saturating_add(num_dissents.saturating_mul(base_refund));

    if let Some(treasury) = sxt_core::utils::account_id_from_table_id::<T>(&quorum.table) {
        let free_balance: u128 =
            polkadot_sdk::pallet_balances::Pallet::<T>::free_balance(&treasury).saturated_into();

        ensure!(
            free_balance > total_needed,
            crate::Error::<T, I>::InsufficientTableFunds
        );

        let agreements: Vec<(T::AccountId, DispatchResult)> = quorum
            .agreements
            .iter()
            .map(|who| {
                (
                    who.clone(),
                    payout_indexer::<T>(&treasury, who, reward_amount),
                )
            })
            .collect();

        let dissents: Vec<(T::AccountId, DispatchResult)> = quorum
            .dissents
            .iter()
            .map(|who| {
                (
                    who.clone(),
                    payout_indexer::<T>(&treasury, who, base_refund),
                )
            })
            .collect();

        Ok(agreements.into_iter().chain(dissents).collect())
    } else {
        Err(crate::Error::<T, I>::InvalidTable)
    }
}

// Helper also used in the balances pallet for this type
fn payout_indexer<T: frame_system::Config + pallet_balances::Config>(
    treasury: &T::AccountId,
    who: &T::AccountId,
    amount: u128,
) -> DispatchResult {
    let indexer_lookup = <T as frame_system::Config>::Lookup::unlookup(who.clone());
    let treasury_lookup = <T as frame_system::Config>::Lookup::unlookup(treasury.clone());

    pallet_balances::Pallet::<T>::force_transfer(
        RawOrigin::Root.into(),
        treasury_lookup,
        indexer_lookup,
        amount.saturated_into(),
    )
}
