//! This module defines the shared application state for the translation layer.
//!
//! The `TranslationLayerState` struct holds instances of key components
//! required for transaction submission, tracking, and interaction with the blockchain node.

use std::sync::Arc;

use subxt::{OnlineClient, PolkadotConfig};
use tokio::sync::Mutex;

use crate::tx_progress::TxProgressDb;
use crate::tx_submitter::TxSubmitter;

/// Represents the shared state for the translation layer.
///
/// This struct encapsulates essential components used throughout the application,
/// including the transaction submitter, transaction progress database, and the
/// Substrate node client. It is wrapped in `Arc` to facilitate safe concurrent access
/// across asynchronous tasks.
///
/// # Fields
/// - `submitter`: A thread-safe, shared transaction submitter for dispatching transactions.
/// - `tx_db`: A database that tracks the status of submitted transactions.
/// - `client`: A Substrate node client for querying blockchain state and submitting extrinsics.
/// ```
#[derive(Clone)]
pub struct TranslationLayerState {
    /// Handles transaction submission to mainnet.
    /// Wrapped in a `Mutex` to ensure safe mutation in an asynchronous environment.
    pub mainnet_submitter: Option<Arc<Mutex<TxSubmitter>>>,

    /// Handles transaction submission to testnet.
    /// Wrapped in a `Mutex` to ensure safe mutation in an asynchronous environment.
    pub testnet_submitter: Option<Arc<Mutex<TxSubmitter>>>,

    /// Stores and tracks transaction progress, ensuring the application can monitor
    /// transaction finality and execution results.
    pub tx_db: Arc<TxProgressDb>,

    /// A client for interacting with the Substrate blockchain.
    /// This client allows querying storage, submitting extrinsics, and fetching chain metadata.
    pub client: Arc<OnlineClient<PolkadotConfig>>,

    /// Network that this state applies to
    pub network: Network,
}

/// Represents the target network environment for the translation layer.
///
/// This enum is used to distinguish between different runtime contexts,
/// such as mainnet and testnet. It helps route requests, select the appropriate
/// transaction submitter, and label metrics or logs accordingly.
///
/// # Variants
/// - `Mainnet`: Indicates that the state or operation is associated with the main production network.
/// - `Testnet`: Indicates that the state or operation is associated with a test or development network.
#[derive(Clone)]
pub enum Network {
    /// SxT Mainnet
    Mainnet,
    /// SxT Testnet
    Testnet,
}
