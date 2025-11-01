use frame_support::dispatch::DispatchInfo;
use frame_support::pallet_prelude::*;
use frame_support::traits::{Currency, Imbalance};
use pallet_transaction_payment::{OnChargeTransaction, RuntimeDispatchInfo};
use sp_runtime::traits::DispatchInfoOf;
use sp_runtime::SaturatedConversion;

use crate::{Balance, RuntimeCall, Vec, DOLLARS};

pub(crate) fn special_fee_for_all_calls(calls: Vec<RuntimeCall>) -> Option<Balance> {
    calls
        .iter()
        .filter_map(|call: &RuntimeCall| special_fee_for_call(call))
        .reduce(|a, b| a + b)
}

pub(crate) fn special_fee_for_call(call: &RuntimeCall) -> Option<Balance> {
    // 20 SXT fee per table
    let fee_per_table = DOLLARS * 20;

    match call.clone() {
        RuntimeCall::Utility(pallet_utility::Call::batch { calls }) => {
            special_fee_for_all_calls(calls)
        }
        RuntimeCall::Utility(pallet_utility::Call::batch_all { calls }) => {
            special_fee_for_all_calls(calls)
        }
        RuntimeCall::Utility(pallet_utility::Call::force_batch { calls }) => {
            special_fee_for_all_calls(calls)
        }
        RuntimeCall::Tables(pallet_tables::Call::create_tables { tables }) => {
            Some(fee_per_table.saturating_mul(tables.len().saturated_into()))
        }
        RuntimeCall::Tables(pallet_tables::Call::create_tables_with_snapshot_and_commitment {
            tables,
            ..
        }) => Some(fee_per_table.saturating_mul(tables.len().saturated_into())),
        RuntimeCall::Tables(pallet_tables::Call::create_namespace { .. }) => Some(fee_per_table),
        _ => None,
    }
}

/// A wrapper for custom gas fees that takes an Inner fallback
pub struct CustomGasFees<C, Inner>(sp_std::marker::PhantomData<(C, Inner)>);

impl<T, C, Fallback> OnChargeTransaction<T> for CustomGasFees<C, Fallback>
where
    T: frame_system::Config<RuntimeCall = RuntimeCall>,
    T: pallet_transaction_payment::Config,
    C: Currency<T::AccountId, Balance = Balance>,
    Fallback:
        OnChargeTransaction<T, LiquidityInfo = Option<C::NegativeImbalance>, Balance = Balance>,
{
    type Balance = Balance;
    type LiquidityInfo = Option<C::NegativeImbalance>;

    fn withdraw_fee(
        who: &T::AccountId,
        call: &T::RuntimeCall,
        info: &DispatchInfoOf<T::RuntimeCall>,
        fee: Self::Balance,
        tip: Self::Balance,
    ) -> Result<Self::LiquidityInfo, TransactionValidityError> {
        // Calculate any special fees from table or namespace creation
        if let Some(special_fee) = special_fee_for_call(call) {
            // Attempt to perform the withdrawal, throwing an error if the account doesn't have the
            // available funds
            Ok(Some(
                <C as Currency<_>>::withdraw(
                    who,
                    special_fee,
                    frame_support::traits::WithdrawReasons::FEE,
                    frame_support::traits::ExistenceRequirement::KeepAlive,
                )
                .map_err(|_| {
                    TransactionValidityError::Invalid(
                        sp_runtime::transaction_validity::InvalidTransaction::Payment,
                    )
                })?,
            ))
        } else {
            // Calculate and withdraw the base fee for the transaction
            Fallback::withdraw_fee(who, call, info, fee, tip)
        }
    }

    fn correct_and_deposit_fee(
        who: &<T>::AccountId,
        dispatch_info: &sp_runtime::traits::DispatchInfoOf<<T>::RuntimeCall>,
        post_info: &sp_runtime::traits::PostDispatchInfoOf<<T>::RuntimeCall>,
        corrected_fee: Self::Balance,
        tip: Self::Balance,
        already_withdrawn: Self::LiquidityInfo,
    ) -> Result<(), TransactionValidityError> {
        // Force the user to always pay the estimate so that custom fees are respected
        let final_fee = if let Some(ref paid) = already_withdrawn {
            paid.peek()
        } else {
            corrected_fee
        };

        Fallback::correct_and_deposit_fee(
            who,
            dispatch_info,
            post_info,
            final_fee,
            tip,
            already_withdrawn,
        )
    }
}
