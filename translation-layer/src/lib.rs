//! # Substrate Transaction Utility Library
//!
//! This library provides utilities for interacting with a Substrate-based blockchain,
//! including transaction submission, table management, and key signing.

/// Error handling module.
///
/// Defines a custom error type using the `snafu` crate to provide detailed and structured
/// error messages for various failures encountered when interacting with the blockchain.
pub mod error;

/// Cryptographic signer module.
///
/// Provides functionality for loading substrate keys from disks.
pub mod signer;

/// Table builder module.
///
/// Offers a builder pattern for constructing and managing tables in the blockchain's storage,
/// allowing users to define table structures and configurations before submitting them.
pub mod table_builder;

/// Transaction submission module.
///
/// Manages the process of submitting transactions, handling retries, tracking nonces,
/// and monitoring transaction progress within the blockchain.
pub mod tx_submitter;

/// Tx progress submitter
pub mod tx_progress;

/// Api
pub mod api;

/// Data models
pub mod model;

/// Utils
pub mod utils;

/// Represents the state of the api
pub mod state;
