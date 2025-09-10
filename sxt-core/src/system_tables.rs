use codec::{Decode, Encode, MaxEncodedLen};
use frame_system::pallet_prelude::BlockNumberFor;
use scale_info::TypeInfo;
use sp_core::RuntimeDebug;
use sp_runtime_interface::pass_by::PassByCodec;

use crate::system_contracts::ContractInfo;

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default)]
pub struct ClaimedUnstake<T>
where
    T: pallet_staking::Config,
{
    pub staker: T::AccountId,
    pub claim_block_number: BlockNumberFor<T>,
    pub claimed_amount: T::CurrencyBalance,
}
