use codec::{Decode, Encode, FullCodec};
use scale_info::TypeInfo;

/// Unstakes that have been claimed through the system-tables pallet.
#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq, Default, TypeInfo)]
pub struct ClaimedUnstake<AccountId, BlockNumber, CurrencyBalance>
where
    AccountId: FullCodec,
    BlockNumber: FullCodec,
    CurrencyBalance: FullCodec,
{
    /// The staker that is claiming their unstake amount.
    pub staker: AccountId,
    /// The block that the claim occurred in.
    pub claim_block_number: BlockNumber,
    /// The unstake amount that was claimed.
    pub claimed_amount: CurrencyBalance,
}
