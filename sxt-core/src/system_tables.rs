use codec::{Decode, Encode, FullCodec};
use scale_info::TypeInfo;

#[derive(Clone, Encode, Decode, Eq, PartialEq, Default, TypeInfo)]
pub struct ClaimedUnstake<AccountId, BlockNumber, CurrencyBalance>
where
    AccountId: FullCodec,
    BlockNumber: FullCodec,
    CurrencyBalance: FullCodec,
{
    pub staker: AccountId,
    pub claim_block_number: BlockNumber,
    pub claimed_amount: CurrencyBalance,
}
