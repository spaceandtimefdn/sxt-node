use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use sp_core::{H160, U256};

/// Basic information about a smart contract stored in the system contracts pallet.
#[derive(
    Copy,
    Clone,
    PartialEq,
    Eq,
    Debug,
    Default,
    Encode,
    Decode,
    MaxEncodedLen,
    TypeInfo,
    Serialize,
    Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct ContractInfo {
    /// Id of the chain that this contract is deployed to.
    pub chain_id: U256,
    /// Address of this contract.
    pub address: H160,
}
