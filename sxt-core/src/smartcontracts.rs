//! # Smart Contracts Core Types
//!
//! This module defines the core types and structures for managing smart contracts within the pallet.
//! It provides representations for different contract types, their details, and associated metadata,
//! using bounded vectors to ensure storage efficiency and enforce length constraints.
//!
//! ## Key Features
//! - **Support for Normal and Proxy Contracts**: Differentiates between standard contracts and proxy contracts with implementation references.
//! - **Bounded Storage Types**: Uses `BoundedVec` for contract addresses and ABI to ensure storage limits are respected.
//! - **Comprehensive Metadata**: Includes details such as contract ABI, starting block, and source chain.
//!
//! ## Types and Structures
//! - [`ContractAddress`]: A bounded vector of up to 64 bytes representing a smart contract's unique address.
//! - [`ContractABI`]: A bounded vector of up to 256 bytes representing the ABI of the smart contract.
//! - [`Contract`]: Enum representing a smart contract, either a `Normal` or `Proxy` contract.
//! - [`ContractDetails`]: A struct containing detailed metadata about a smart contract.
//! - [`NormalContract`]: A struct representing a standard (non-proxy) smart contract.
//! - [`ProxyContract`]: A struct representing a proxy smart contract with an associated implementation contract.
//! - [`ImplementationContract`]: A struct representing the implementation contract details used by a proxy contract.

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::storage::bounded_vec::BoundedVec;
use frame_support::traits::ConstU32;
use scale_info::TypeInfo;
use sp_core::RuntimeDebug;

use crate::tables::{CreateStatement, Source};
use crate::ByteString;

/// A bounded vector representing a smart contract's unique address.
///
/// This is stored as a byte array with a maximum length of 64 bytes.
pub type ContractAddress = BoundedVec<u8, ConstU32<64>>;

/// A bounded vector representing the ABI (Application Binary Interface) of a smart contract.
///
/// This is stored as a byte array with a maximum length of 256 bytes.
pub type ContractABI = BoundedVec<u8, ConstU32<256>>;

/// Represents a smart contract, which can either be:
/// - `Normal`: A standard smart contract.
/// - `Proxy`: A proxy smart contract that points to an implementation contract.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum Contract {
    /// A standard smart contract.
    Normal(NormalContract),

    /// A proxy smart contract with a reference to an implementation contract.
    Proxy(ProxyContract),
}

/// Represents a standard (non-proxy) smart contract.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct NormalContract {
    /// The details of the normal contract, including address, ABI, and metadata.
    pub details: ContractDetails,
}

/// Represents an implementation contract used by a proxy contract.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct ImplementationContract {
    /// The details of the implementation contract, including address, ABI, and metadata.
    pub details: ContractDetails,
}

/// Represents a proxy smart contract.
///
/// A proxy contract delegates its functionality to an implementation contract.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct ProxyContract {
    /// The details of the proxy contract, including address and metadata.
    pub details: ContractDetails,

    /// The implementation contract that the proxy contract points to.
    pub implementation: ImplementationContract,
}

/// Detailed metadata about a smart contract.
///
/// This struct includes information about the contract's source chain, address, ABI, and other relevant metadata.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct ContractDetails {
    /// The source chain where the contract is deployed (e.g., Ethereum, Bitcoin).
    pub source: Source,

    /// The unique address of the contract.
    pub address: ContractAddress,

    /// The ABI (Application Binary Interface) of the contract, if available.
    pub abi: Option<ContractABI>,

    /// The starting block of the contract, if applicable.
    pub starting_block: Option<u64>,

    /// The target schema for the contract, if applicable.
    pub target_schema: Option<CreateStatement>,

    /// The name of the contract, if available.
    pub contract_name: Option<ByteString>,
}
