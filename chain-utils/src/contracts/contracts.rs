//! This module contains definitions for the Mainnet System Contracts

use sxt_core::sxt_chain_runtime::api::runtime_types::bounded_collections::bounded_vec::BoundedVec;
use sxt_core::sxt_chain_runtime::api::runtime_types::proof_of_sql_commitment_map::commitment_scheme::CommitmentSchemeFlags;
use sxt_core::sxt_chain_runtime::api::runtime_types::sxt_core::smartcontracts::{Contract, ContractDetails, EventDetails, NormalContract};
use sxt_core::sxt_chain_runtime::api::runtime_types::sxt_core::tables::Source;

use crate::contracts::abis::{zkpay_abi, staking_abi};

pub fn devnet_messaging() -> Contract {
    messaging_contract("433e5d1919dc9c4fe51782416780443e88dd6979", Source::Sepolia, 8166436)
}

pub fn devnet_staking() -> Contract {
    staking_contract("425676FB4ee9763f90a2e103A70784595AabB783", Source::Sepolia, 8166436)
}

pub fn devnet_zkpay() -> Contract {
    testnet_zkpay()
}

pub fn testnet_messaging() -> Contract {
    messaging_contract("5FFDa3bd0D4aa3FC1C2CF83F34b0eF1d9D89A118", Source::Sepolia, 8173966)
}

pub fn testnet_staking() -> Contract {
    staking_contract("7B3cBAaFE8Ff3cbf4553893fdcaD8d5c46DB90Ab", Source::Sepolia, 8173966)
}
pub fn testnet_faster_staking() -> Contract {
    staking_contract("0Ed4e306B11349f875B167FcA1C2BC314489234f", Source::Sepolia, 9237050)
}
pub fn testnet_zkpay() -> Contract {
    zkpay_contract("a735143283a6e686723403a820841e5774951a63", Source::Sepolia, 8126513)
}
pub fn mainnet_messaging() -> Contract {
    messaging_contract("70106a3247542069a3ee1AF4D6988a5f34b31cE1", Source::Ethereum, 22347677)
}

pub fn mainnet_staking() -> Contract {
    staking_contract("93d176dd54FF38b08f33b4Fc62573ec80F1da185", Source::Ethereum, 22347700)
}

pub fn mainnet_zkpay() -> Contract {
    zkpay_contract("27d4D2AF364c1ad2eBDB2a28D6cb7B99EdE1D450", Source::Ethereum, 22427605)
}

pub fn messaging_contract(address: &str, source: Source, starting_block: u64) -> Contract {
    Contract::Normal(NormalContract {
        details: ContractDetails {
            source,
            address: BoundedVec(
                hex::decode(address).unwrap(),
            ),
            abi: Some(BoundedVec(
                    br#"[{"anonymous":false,"inputs":[{"indexed":false,"internalType":"address","name":"sender","type":"address"},{"indexed":false,"internalType":"bytes","name":"body","type":"bytes"},{"indexed":false,"internalType":"uint248","name":"nonce","type":"uint248"}],"name":"Message","type":"event"},{"inputs":[{"internalType":"address","name":"sender","type":"address"}],"name":"getNonce","outputs":[{"internalType":"uint248","name":"nonce","type":"uint248"}],"stateMutability":"view","type":"function"},{"inputs":[{"internalType":"bytes","name":"body","type":"bytes"}],"name":"message","outputs":[],"stateMutability":"nonpayable","type":"function"}]"#
                .to_vec(),
            )),
            starting_block: Some(starting_block),
            target_schema: Some(BoundedVec(b"SXT_SYSTEM_STAKING".to_vec())),
            contract_name: Some(BoundedVec(b"SXTChainMessaging".to_vec())),
            event_details: Some(BoundedVec(vec![
                EventDetails {
                    name: BoundedVec(b"Message".to_vec()),
                    signature: BoundedVec(b"Message(address sender, bytes body, uint248 nonce)".to_vec()),
                    table: BoundedVec(b"MESSAGE".to_vec())
                }
            ])),
            ddl_statement: Some(BoundedVec(
                b"CREATE SCHEMA IF NOT EXISTS SXT_SYSTEM_STAKING".to_vec(),
            )),
        },
    })
}


pub fn zkpay_contract(address: &str, source: Source, starting_block: u64) -> Contract {
    Contract::Normal(NormalContract {
        details: ContractDetails { 
            source,
            address: BoundedVec(
                hex::decode(address).unwrap()
            ),
            abi: Some(zkpay_abi()),
            starting_block: Some(starting_block),
            target_schema: Some(BoundedVec(b"SXT_SYSTEM_ZKPAY".to_vec())),
            contract_name: Some(BoundedVec(b"ZKPayV2".to_vec())),
            event_details: Some(BoundedVec(vec![
                EventDetails {
                    name: BoundedVec(b"AssetAdded".to_vec()),
                    signature: BoundedVec(b"AssetAdded(address asset, bytes1 allowedPaymentTypes, address priceFeed, uint8 tokenDecimals, uint64 stalePriceThresholdInSeconds)".to_vec()),
                    table: BoundedVec(b"ASSET_ADDED".to_vec()),
                },
                EventDetails {
                    name: BoundedVec(b"AssetRemoved".to_vec()),
                    signature: BoundedVec(b"AssetRemoved(address asset)".to_vec()),
                    table: BoundedVec(b"ASSET_REMOVED".to_vec()),
                },
                EventDetails {
                    name: BoundedVec(b"CallbackFailed".to_vec()),
                    signature: BoundedVec(b"CallbackFailed(bytes32 queryHash, address callbackClientContractAddress)".to_vec()),
                    table: BoundedVec(b"CALLBACK_FAILED".to_vec()),
                },
                EventDetails {
                    name: BoundedVec(b"CallbackSucceeded".to_vec()),
                    signature: BoundedVec(b"CallbackSucceeded(bytes32 queryHash, address callbackClientContractAddress)".to_vec()),
                    table: BoundedVec(b"CALLBACK_SUCCEEDED".to_vec()),
                },
                EventDetails {
                    name: BoundedVec(b"Initialized".to_vec()),
                    signature: BoundedVec(b"Initialized(uint64 version)".to_vec()),
                    table: BoundedVec(b"INITIALIZED".to_vec()),
                },
                EventDetails {
                    name: BoundedVec(b"NewQueryPayment".to_vec()),
                    signature: BoundedVec(b"NewQueryPayment(bytes32 queryHash, address asset, uint248 amount, address source, uint248 amountInUSD)".to_vec()),
                    table: BoundedVec(b"NEW_QUERY_PAYMENT".to_vec()),
                },
                EventDetails {
                    name: BoundedVec(b"OwnershipTransferred".to_vec()),
                    signature: BoundedVec(b"OwnershipTransferred(address previousOwner, address newOwner)".to_vec()),
                    table: BoundedVec(b"OWNERSHIP_TRANSFERRED".to_vec()),
                },
                EventDetails {
                    name: BoundedVec(b"PaymentRefunded".to_vec()),
                    signature: BoundedVec(b"PaymentRefunded(bytes32 queryHash, address asset, address source, uint248 amount)".to_vec()),
                    table: BoundedVec(b"PAYMENT_REFUNDED".to_vec()),
                },
                EventDetails {
                    name: BoundedVec(b"PaymentSettled".to_vec()),
                    signature: BoundedVec(b"PaymentSettled(bytes32 queryHash, uint248 usedAmount, uint248 remainingAmount)".to_vec()),
                    table: BoundedVec(b"PAYMENT_SETTLED".to_vec()),
                },
                EventDetails {
                    name: BoundedVec(b"QueryCanceled".to_vec()),
                    signature: BoundedVec(b"QueryCanceled(bytes32 queryHash, address caller)".to_vec()),
                    table: BoundedVec(b"QUERY_CANCELED".to_vec()),
                },
                EventDetails {
                    name: BoundedVec(b"QueryFulfilled".to_vec()),
                    signature: BoundedVec(b"QueryFulfilled(bytes32 queryHash)".to_vec()),
                    table: BoundedVec(b"QUERY_FULFILLED".to_vec()),
                },
                EventDetails {
                    name: BoundedVec(b"QueryReceived".to_vec()),
                    signature: BoundedVec(b"QueryReceived(uint248 queryNonce, address sender, bytes query, bytes queryParameters, uint64 timeout, address callbackClientContractAddress, uint64 callbackGasLimit, bytes callbackData, address customLogicContractAddress, bytes32 queryHash)".to_vec()),
                    table: BoundedVec(b"QUERY_RECEIVED".to_vec()),
                },
                EventDetails {
                    name: BoundedVec(b"SendPayment".to_vec()),
                    signature: BoundedVec(b"SendPayment(address asset, uint248 amount, bytes32 onBehalfOf, address target, bytes memo, uint248 amountInUSD, address sender)".to_vec()),
                    table: BoundedVec(b"SEND_PAYMENT".to_vec()),
                },
                EventDetails {
                    name: BoundedVec(b"TreasurySet".to_vec()),
                    signature: BoundedVec(b"TreasurySet(address treasury)".to_vec()),
                    table: BoundedVec(b"TREASURY_SET".to_vec()),
                },
            ])),
            ddl_statement: Some(BoundedVec(
                b"CREATE SCHEMA IF NOT EXISTS SXT_SYSTEM_ZKPAY".to_vec()
            )),
        }
    })
}

pub fn staking_contract(address: &str, source: Source, starting_block: u64) -> Contract {
    Contract::Normal(NormalContract {
        details: ContractDetails {
            source,
            address: BoundedVec(hex::decode(address).unwrap()),
            abi: Some(staking_abi()),
            starting_block: Some(starting_block),
            target_schema: Some(BoundedVec(b"SXT_SYSTEM_STAKING".to_vec())),
            contract_name: Some(BoundedVec(b"Staking".to_vec())),
            event_details: Some(BoundedVec(vec![
                EventDetails {
                    name: BoundedVec(b"Staked".to_vec()),
                    signature: BoundedVec(b"Staked(address staker,uint248 amount)".to_vec()),
                    table: BoundedVec(b"STAKED".to_vec()),
                },
                EventDetails {
                    name: BoundedVec(b"UnstakeInitiated".to_vec()),
                    signature: BoundedVec(
                        b"UnstakeInitiated(address staker,uint248 amount)".to_vec(),
                    ),
                    table: BoundedVec(b"UNSTAKEINITIATED".to_vec()),
                },
                EventDetails {
                    name: BoundedVec(b"UnstakeClaimed".to_vec()),
                    signature: BoundedVec(b"UnstakeClaimed(address staker)".to_vec()),
                    table: BoundedVec(b"UNSTAKECLAIMED".to_vec()),
                },
                EventDetails {
                    name: BoundedVec(b"Unstaked".to_vec()),
                    signature: BoundedVec(b"Unstaked(address staker,uint248 amount)".to_vec()),
                    table: BoundedVec(b"UNSTAKED".to_vec()),
                },
                EventDetails {
                    name: BoundedVec(b"Nominated".to_vec()),
                    signature: BoundedVec(
                        b"Nominated(bytes32[] nodesEd25519PubKeys,address nominator)".to_vec(),
                    ),
                    table: BoundedVec(b"NOMINATED".to_vec()),
                },
                EventDetails {
                    name: BoundedVec(b"InitiateUnstakeCancelled".to_vec()),
                    signature: BoundedVec(b"InitiateUnstakeCancelled(address staker)".to_vec()),
                    table: BoundedVec(b"INITIATEUNSTAKECANCELLED".to_vec()),
                },
                EventDetails {
                    name: BoundedVec(b"Paused".to_vec()),
                    signature: BoundedVec(b"Paused(address account)".to_vec()),
                    table: BoundedVec(b"PAUSED".to_vec()),
                },
                EventDetails {
                    name: BoundedVec(b"Unpaused".to_vec()),
                    signature: BoundedVec(b"Unpaused(address account)".to_vec()),
                    table: BoundedVec(b"UNPAUSED".to_vec()),
                },
                EventDetails {
                    name: BoundedVec(b"OwnershipTransferred".to_vec()),
                    signature: BoundedVec(
                        b"OwnershipTransferred(address previousOwner,address newOwner)".to_vec(),
                    ),
                    table: BoundedVec(b"OWNERSHIPTRANSFERRED".to_vec()),
                },
                EventDetails {
                    name: BoundedVec(b"UnstakingUnbondingPeriodSet".to_vec()),
                    signature: BoundedVec(
                        b"UnstakingUnbondingPeriodSet(uint64 unstakingUnbondingPeriod)".to_vec(),
                    ),
                    table: BoundedVec(b"UNSTAKINGUNBONDINGPERIODSET".to_vec()),
                },
                EventDetails {
                    name: BoundedVec(b"StakingPoolSet".to_vec()),
                    signature: BoundedVec(b"StakingPoolSet(address stakingPool)".to_vec()),
                    table: BoundedVec(b"STAKINGPOOLSET".to_vec()),
                },
                EventDetails {
                    name: BoundedVec(b"StakingTokenSet".to_vec()),
                    signature: BoundedVec(b"StakingTokenSet(address token)".to_vec()),
                    table: BoundedVec(b"STAKINGTOKENSET".to_vec()),
                },
                EventDetails {
                    name: BoundedVec(b"SubstrateSignatureValidatorSet".to_vec()),
                    signature: BoundedVec(
                        b"SubstrateSignatureValidatorSet(address substrateSignatureValidator)"
                            .to_vec(),
                    ),
                    table: BoundedVec(b"SUBSTRATESIGNATUREVALIDATORSET".to_vec()),
                },
            ])),
            ddl_statement: Some(BoundedVec(
                b"CREATE SCHEMA IF NOT EXISTS SXT_SYSTEM_STAKING".to_vec(),
            )),
        },
    })
}
