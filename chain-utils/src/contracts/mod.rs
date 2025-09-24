use sxt_core::sxt_chain_runtime::api::runtime_types::sxt_core::smartcontracts::Contract;
mod abis;
pub mod contracts;

/// SXT Networks
#[derive(Clone, Copy, Eq, PartialEq, clap::ValueEnum, Debug)]
pub enum SxtNetwork {
    Mainnet,
    Testnet,
    Devnet,
}

/// The different system contracts supported by SXT Chain
#[derive(Clone, Copy, Eq, PartialEq, clap::ValueEnum, Debug)]
pub enum SystemContract {
    Staking,
    Messaging,
    ZkPay,
}

pub fn get_contract(network: SxtNetwork, contract: SystemContract) -> Contract {
    match (network, contract) {
        (SxtNetwork::Mainnet, SystemContract::ZkPay) => contracts::mainnet_zkpay(),
        (SxtNetwork::Mainnet, SystemContract::Staking) => contracts::mainnet_staking(),
        (SxtNetwork::Mainnet, SystemContract::Messaging) => contracts::mainnet_messaging(),
        (SxtNetwork::Testnet, SystemContract::ZkPay) => contracts::mainnet_zkpay(),
        // (SxtNetwork::Testnet, SystemContract::Staking) => contracts::testnet_staking(),
        (SxtNetwork::Testnet, SystemContract::Staking) => contracts::testnet_faster_staking(),
        (SxtNetwork::Testnet, SystemContract::Messaging) => contracts::testnet_messaging(),
        (SxtNetwork::Devnet, SystemContract::ZkPay) => contracts::devnet_zkpay(),
        (SxtNetwork::Devnet, SystemContract::Staking) => contracts::devnet_staking(),
        (SxtNetwork::Devnet, SystemContract::Messaging) => contracts::devnet_messaging(),
    }
}
