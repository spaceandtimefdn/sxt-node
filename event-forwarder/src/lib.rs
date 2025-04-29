//! # Event Forwarder Library
//!
//! This library provides the core components for listening to blockchain events,
//! processing attestations, and forwarding relevant data to external systems such as Ethereum smart contracts.
//!
//! ## Modules
//! - [`chain_listener`]: Manages real-time block streaming and event processing from the blockchain.
//! - [`event_forwarder`]: Handles attestation events, staking, unbonding, and interactions with Ethereum smart contracts.
//! - [`kitchen_sink`]: Integration testing framework that verifies end-to-end blockchain interactions.

/// The `chain_listener` module provides a framework for subscribing to blockchain blocks,
/// processing them in real time, and integrating with custom event processors.
pub mod chain_listener;

/// The `event_forwarder` module is responsible for processing attestations and forwarding them
/// to an Ethereum smart contract, ensuring the integrity of staking and Merkle tree proofs.
pub mod event_forwarder;

/// The event forwarder contract built with sol apis.
pub mod event_forwarder_contract;

/// The `kitchen_sink` module serves as an integration testing suite, ensuring that blockchain events
/// are properly processed and forwarded in a full end-to-end setup.
pub mod kitchen_sink;

/// The `block_processing` module provides utilities for extracting, validating,
/// and processing blockchain events from finalized blocks.
///
/// This module handles the retrieval of attestations, staking events, and unbonding events,
/// ensuring that relevant data is properly processed and forwarded. Additionally, it includes
/// nonce management and transaction submission with retry logic for robust execution.
///
/// ## Features
/// - **Event Extraction**: Fetches attestation and unbonding events from blocks.
/// - **Merkle Tree Construction**: Builds cryptographic proofs for staking-related data.
/// - **Transaction Submission**: Handles nonce management and retries for marking blocks as forwarded.
/// - **Error Handling**: Provides structured error types to ensure resilience in blockchain interactions.
///
/// ## Key Functions
/// - [`fetch_attested_block`] - Retrieves the full block referenced in an attestation event.
/// - [`fetch_block_attestations`] - Extracts attestation events from a block.
/// - [`fetch_unbonding_events`] - Retrieves staking unbonding events from a block.
/// - [`build_merkle_tree`] - Constructs a Merkle tree from commitments and accounts.
/// - [`mark_block_forwarded`] - Submits a transaction to confirm that a block has been processed.
///
/// This module is primarily used by the [`event_forwarder`] to process blockchain attestations
/// before forwarding them to external systems.
pub mod block_processing;
