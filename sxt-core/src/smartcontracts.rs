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

use crate::tables::{CreateStatement, Source, TableName};
use crate::{ByteString, IdentLength};

/// A bounded vector representing a smart contract's unique address.
///
/// This is stored as a byte array with a maximum length of 64 bytes.
pub type ContractAddress = BoundedVec<u8, ConstU32<64>>;

/// A bounded vector representing the ABI (Application Binary Interface) of a smart contract.
///
/// This is stored as a byte array with a maximum length of 32,768 bytes.
pub type ContractABI = BoundedVec<u8, ConstU32<32_768>>;

/// Represents a smart contract, which can either be:
/// - `Normal`: A standard smart contract.
/// - `Proxy`: A proxy smart contract that points to an implementation contract.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[allow(clippy::large_enum_variant)]
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
    pub target_schema: Option<ByteString>,

    /// The name of the contract, if available.
    pub contract_name: Option<ByteString>,

    /// A list of event details associated with the contract.
    pub event_details: Option<EventDetailsList>,

    /// DDL statement
    pub ddl_statement: Option<CreateStatement>,
}

/// A bounded vector representing an Ethereum-compatible event signature.
///
/// Ethereum event signatures follow the format:
/// ```solidity
/// EventName(Type1 indexed param1, Type2 param2, ...)
/// ```
/// The maximum estimated length is:
/// - Event name: ~64 characters
/// - Parameters: ~450 characters (assuming multiple indexed and complex types)
/// - Formatting (commas, spaces): ~30 characters
///
/// **Total upper bound: ~550 characters**  
/// We set a safe limit of **600 bytes** for future-proofing.
pub type EventSignature = BoundedVec<u8, ConstU32<600>>;

/// A bounded vector representing an event name.
///
/// This name should follow Solidity-compatible identifier conventions.
/// The length limit is defined by `IdentLength`.
pub type EventName = BoundedVec<u8, IdentLength>;

/// Represents detailed information about an individual smart contract event.
///
/// Each event maps an on-chain emitted event to a structured table in an off-chain database.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct EventDetails {
    /// The event's name (e.g., `Transfer`, `Approval`).
    pub name: EventName,

    /// The full event signature, including parameter types.
    pub signature: EventSignature,

    /// The target table where event data should be stored.
    pub table: TableName,
}

/// A bounded list of event details.
///
/// This list holds up to **100** event mappings for a single contract.
/// A reasonable upper bound is set to prevent excessive storage usage.
pub type EventDetailsList = BoundedVec<EventDetails, ConstU32<100>>;
