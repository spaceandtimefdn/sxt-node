//! # Attestation Processing Module
//!
//! This module provides functionality for fetching and processing blockchain attestations,
//! managing Merkle tree proofs, and interacting with Ethereum smart contracts. It also
//! includes retry logic for submitting transactions to ensure robustness in network failures.
//!
//! ## Features:
//!
//! - Fetching attestation and staking-related events from blocks.
//! - Constructing Merkle trees and generating cryptographic proofs.
//! - Handling Ethereum-based attestations with ECDSA signatures.
//! - Managing nonce-based transaction submission with retries.
//!
//! ## Usage
//!
//! This module is intended for use in a blockchain listener that processes events
//! and forwards attestation data to an Ethereum contract. It interacts with Substrate
//! via `subxt` and Ethereum via `alloy`.
//!
//! ## Important Components
//!
//! ### Fetching Blockchain Events
//! - [`fetch_attested_block`] - Retrieves the attested block associated with an attestation.
//! - [`fetch_block_attestations`] - Fetches attestation events from a block.
//! - [`fetch_unbonding_events`] - Fetches unbonding events from a block.
//!
//! ### Merkle Tree Processing
//! - [`convert_proof`] - Converts hex-encoded proofs to `FixedBytes<32>` format.
//!
//! ### Transaction Handling
//! - [`mark_block_forwarded`] - Marks a block as forwarded with retry logic.
//! - [`send_channel_update`] - Sends an update signal via a channel after processing a block.
//!
//! ## Error Handling
//!
//! This module defines a comprehensive error type [`Error`] using `snafu` to ensure
//! detailed and structured error messages for various failure cases, including blockchain
//! fetching failures, attestation validation errors, and transaction submission issues.
//!

use alloy::primitives::FixedBytes;
use eth_merkle_tree::utils::errors::BytesError;
use log::{error, info, warn};
use snafu::{ResultExt, Snafu};
use sp_core::crypto::AccountId32;
use subxt::config::polkadot::PolkadotExtrinsicParamsBuilder as Params;
use subxt_signer::sr25519::Keypair;
use sxt_core::sxt_chain_runtime::api::attestations::events::BlockAttested;
use sxt_core::sxt_chain_runtime::api::runtime_types::sxt_core::attestation::Attestation::EthereumAttestation;
use sxt_core::sxt_chain_runtime::api::staking::events::Unbonded;
use sxt_core::sxt_chain_runtime::{self};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use watcher::attestation;

use crate::chain_listener::{Block, API};

/// Fetches the block that was attested.
///
/// This function retrieves the full block corresponding to an attestation event.
/// It queries the blockchain using the `block_hash` stored in the attestation.
///
/// # Arguments
/// - `api`: A reference to the blockchain API.
/// - `attestation`: The attestation event containing the attested block details.
///
/// # Returns
/// - `Ok(Block)`: The full attested block if found.
/// - `Err(Error::FetchAttestedBlock)`: If the block retrieval fails.
pub async fn fetch_attested_block(api: &API, attestation: &BlockAttested) -> Result<Block, Error> {
    match &attestation.attestation {
        EthereumAttestation {
            block_number,
            block_hash,
            ..
        } => api
            .blocks()
            .at(*block_hash)
            .await
            .context(FetchAttestedBlockSnafu {
                block_number: *block_number,
            }),
    }
}

/// Fetches events of type `T` from a block.
pub async fn fetch_events<T: subxt::events::StaticEvent + 'static>(
    block: &Block,
) -> Result<Vec<T>, Error> {
    Ok(block
        .events()
        .await
        .context(FetchAttestationSnafu)?
        .iter()
        .flatten()
        .filter_map(|event| event.as_event::<T>().ok().flatten())
        .collect::<Vec<_>>())
}

/// Fetches attestation events from a given block.
///
/// # Parameters
/// - `block`: The block from which to fetch attestations.
///
/// # Returns
/// - `Ok(Vec<BlockAttested>)` if attestation events were successfully retrieved.
/// - `Err(Error::FetchAttestation)` if an error occurs while fetching events.
pub async fn fetch_block_attestations(block: &Block) -> Result<Vec<BlockAttested>, Error> {
    fetch_events::<BlockAttested>(block).await
}

/// Fetches unbonding events from a block.
pub async fn fetch_unbonding_events(block: &Block) -> Result<Vec<Unbonded>, Error> {
    fetch_events::<Unbonded>(block).await
}

/// Converts a list of hex-encoded Merkle proof elements into `FixedBytes<32>`.
///
/// This function ensures each proof element is correctly formatted, decodes the hex,
/// and converts it into a fixed-length byte array.
///
/// # Arguments
/// - `proof_strings`: A vector of hex-encoded 32-byte proof elements.
///
/// # Returns
/// - `Ok(Vec<FixedBytes<32>>)` on success.
/// - `Err(Box<dyn std::error::Error>)` if the conversion fails due to formatting errors.
///
/// # Errors
/// - If a proof element is not exactly 64 hex characters.
/// - If decoding the hex string fails.
/// - If the resulting byte slice is not exactly 32 bytes.
pub fn convert_proof(
    proof_strings: Vec<String>,
) -> Result<Vec<FixedBytes<32>>, Box<dyn std::error::Error>> {
    proof_strings
        .into_iter()
        .map(|mut hex_str| {
            if hex_str.starts_with("0x") {
                hex_str = hex_str.trim_start_matches("0x").to_string();
            }

            if hex_str.len() != 64 {
                error!(
                    "Invalid proof length: Expected 64 hex chars, got {}",
                    hex_str.len()
                );
                return Err("Invalid proof length".into());
            }

            let decoded_bytes = hex::decode(&hex_str).inspect_err(|e| {
                error!("Failed to decode hex string '{}': {}", hex_str, e);
            })?;

            let fixed_bytes: [u8; 32] = decoded_bytes.try_into().map_err(|_| {
                error!("Proof entry is not 32 bytes long: {}", hex_str);
                "Invalid proof length"
            })?;

            Ok(FixedBytes::<32>::from(fixed_bytes)) // Convert to FixedBytes<32>
        })
        .collect()
}

/// Maximum retry attempts for forwarding tx
const MAX_RETRIES: usize = 3;

/// Submits a transaction to mark a block as forwarded, retrying if needed.
///
/// This function attempts to submit a transaction up to a maximum number of retries.
/// It uses an incremented nonce for each attempt and applies exponential backoff to
/// reduce the risk of nonce conflicts.
///
/// # Arguments
/// - `api`: A reference to the blockchain API.
/// - `block_number`: The block number being marked as forwarded.
/// - `keypair`: The signing keypair for the transaction.
/// - `initial_nonce`: The starting nonce for the transaction.
///
/// # Returns
/// - `Ok(u64)`: The updated nonce after a successful transaction.
/// - `Err(Error::BlockForwardingError)`: If all attempts fail.
///
/// # Errors
/// - If the transaction fails after the maximum number of retries.
pub async fn mark_block_forwarded(
    api: &API,
    block_number: u32,
    keypair: &Keypair,
    initial_nonce: u64,
) -> Result<u64, Error> {
    for attempt in 0..=MAX_RETRIES {
        let nonce_value = initial_nonce + attempt as u64; // Increment nonce for each retry
        let tx_params = Params::new().nonce(nonce_value).build();

        let forwarded_block_tx = sxt_chain_runtime::api::tx()
            .attestations()
            .mark_block_forwarded(block_number);

        match api
            .tx()
            .sign_and_submit_then_watch(&forwarded_block_tx, keypair, tx_params)
            .await
        {
            Ok(_) => {
                info!(
                    "✅ Successfully marked block {} as forwarded on attempt {}",
                    block_number,
                    attempt + 1
                );
                return Ok(nonce_value + 1);
            }
            Err(err) if attempt < MAX_RETRIES => {
                warn!(
                    "⚠️ Attempt {} failed for block {}: {}. Retrying...",
                    attempt + 1,
                    block_number,
                    err
                );
                sleep(Duration::from_millis(500 * (attempt as u64 + 1))).await; // Exponential backoff
            }
            Err(err) => {
                error!(
                    "❌ Failed to mark block {} as forwarded after {} attempts: {}",
                    block_number,
                    MAX_RETRIES + 1,
                    err
                );
                return Err(Error::BlockForwardingError { source: err });
            }
        }
    }

    Err(Error::BlockForwardingError {
        source: subxt::Error::Other("Unexpected transaction failure".into()),
    })
}

/// Sends a channel update signal to notify about the processing status.
///
/// This function is used to inform an external listener that a block has been processed.
/// If the channel is `None`, the function exits silently.
///
/// # Arguments
/// - `channel`: An optional sender channel to send the update signal.
///
/// # Errors
/// - If sending the signal fails, an error is logged, but no panic occurs.
pub async fn send_channel_update(channel: &Option<mpsc::Sender<bool>>) {
    if let Some(sender) = channel {
        if let Err(e) = sender.send(true).await {
            error!("Failed to send channel update: {}", e);
        }
    }
}

/// Defines possible errors encountered while processing blockchain events and interacting with Ethereum.
#[derive(Debug, Snafu)]
pub enum Error {
    /// Error fetching attestation events from a block.
    ///
    /// This occurs when attempting to extract attestation events from a blockchain block,
    /// but an issue arises in the process.
    ///
    /// - **Cause:** An error in the blockchain query or event decoding.
    /// - **Solution:** Ensure the block contains attestation events and that event parsing logic is correct.
    #[snafu(display("Failed to fetch attestation events: {source}"))]
    FetchAttestation {
        /// source error
        source: subxt::Error,
    },

    /// Error fetching the attested block.
    ///
    /// This occurs when an attestation event references a block, but the retrieval
    /// of that block fails.
    ///
    /// - **Cause:** The block might not exist, or there could be network issues.
    /// - **Solution:** Verify the attestation event and check blockchain connectivity.
    #[snafu(display("Failed to fetch attested block {}: {source}", block_number))]
    FetchAttestedBlock {
        /// The block number that failed to be fetched.
        block_number: u32,
        /// The underlying error from the blockchain client.
        source: subxt::Error,
    },

    /// Error fetching unbonding events from an attested block.
    ///
    /// This occurs when retrieving unbonding events related to staking withdrawals.
    ///
    /// - **Cause:** The block might not contain unbonding events or there could be an API issue.
    /// - **Solution:** Check if the block is within the staking unbonding period.
    #[snafu(display("Failed to fetch unbonding events: {source}"))]
    FetchUnbonding {
        /// source error
        source: subxt::Error,
    },

    /// Error fetching commitments and accounts for Merkle tree construction.
    ///
    /// - **Cause:** The commitments or accounts data might be unavailable or corrupted.
    /// - **Solution:** Ensure the attestation service correctly logs commitments.
    #[snafu(display("Error fetching commitments and accounts: {source}"))]
    FetchCommitmentsAndAccounts {
        /// source error
        source: attestation::fetch::FetchError,
    },

    /// The Merkle tree has an empty state root.
    ///
    /// - **Cause:** The tree was built with no valid data.
    /// - **Solution:** Verify that commitments and accounts were correctly included in the tree.
    #[snafu(display("Merkle tree calculated an empty state root"))]
    EmptyMerkleRoot,

    /// Error fetching the balance of a given account.
    ///
    /// - **Cause:** The blockchain query for the account's balance failed.
    /// - **Solution:** Ensure the account exists and that the query is correctly formatted.
    #[snafu(display(
        "There was an error fetching the balance for account id: {}, {source}",
        hex::encode(account_id)
    ))]
    FetchBalanceError {
        /// The blockchain account ID that failed to be queried.
        account_id: subxt::utils::AccountId32,
        /// The underlying error from the blockchain client.
        source: subxt::error::Error,
    },

    /// No balance information was found for a specific account.
    ///
    /// - **Cause:** The account does not exist, or there are no funds in it.
    /// - **Solution:** Verify the account ID and check blockchain state.
    #[snafu(display("No balance was fetched for this user: {}", hex::encode(account_id)))]
    NoBalanceError {
        /// The blockchain account ID that could not be found.
        account_id: subxt::utils::AccountId32,
    },

    /// Error converting an account ID to the expected type.
    ///
    /// - **Cause:** The ID format is incompatible or corrupted.
    /// - **Solution:** Ensure the account ID is a valid `AccountId32`.
    #[snafu(display(
        "There was an error converting the account id to the appropriate type: {}",
        hex::encode(account_id)
    ))]
    AccountIdConversionError {
        /// The original account ID that failed to convert.
        account_id: AccountId32,
    },

    /// Error during Keccak hashing.
    ///
    /// - **Cause:** An invalid byte sequence or hashing operation failure.
    /// - **Solution:** Ensure the input data is correctly formatted before hashing.
    #[snafu(display("There was an error hashing the data: {source}"))]
    KeccakError {
        /// The source error
        source: BytesError,
    },

    /// Error locating a specific Merkle tree leaf.
    ///
    /// - **Cause:** The Merkle tree might not contain the given leaf.
    /// - **Solution:** Ensure the correct data was used for Merkle tree insertion.
    #[snafu(display("The leaf was unable to be located"))]
    LocateLeafError,

    /// Error in formatting cryptographic proof data.
    ///
    /// - **Cause:** The proof length is incorrect or formatted improperly.
    /// - **Solution:** Ensure the proof is generated correctly before submitting.
    #[snafu(display("The proof could not be formatted into the proper format"))]
    InvalidProofLength,

    /// Error decoding a state root from a hexadecimal string.
    ///
    /// - **Cause:** The state root is incorrectly formatted or truncated.
    /// - **Solution:** Ensure the state root is encoded in valid hexadecimal format.
    #[snafu(display("Could not decode the state root {}: {source}"))]
    DecodeStateRootError {
        /// The incorrectly formatted state root.
        state_root: String,
        /// The underlying decoding error.
        source: hex::FromHexError,
    },

    /// Error updating the chain's forwarded block counter
    #[snafu(display("Could note submit block forwarded tx to chain: {source}"))]
    BlockForwardingError {
        /// source subxt error
        source: subxt::error::Error,
    },
}
