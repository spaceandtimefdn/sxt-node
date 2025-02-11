//! # Event Forwarder Module
//!
//! This module implements the `EventForwarderProcessor`, which listens to blockchain events,
//! processes attestations, and interacts with the `EventForwarder` contract deployed on Ethereum.
//!
//! ## Features:
//! - Fetching attestations from blocks.
//! - Processing staking and unbonding events.
//! - Constructing Merkle trees and generating cryptographic proofs.
//! - Interacting with Ethereum smart contracts.
//!
//! This module is primarily responsible for processing blockchain data and forwarding it
//! to an Ethereum contract via `alloy` and `subxt` integrations.

use std::sync::Arc;

use alloy::network::EthereumWallet;
use alloy::primitives::{Address, FixedBytes, U256};
use alloy::sol;
use async_trait::async_trait;
use eth_merkle_tree::utils::errors::BytesError;
use eth_merkle_tree::utils::keccak::keccak256;
use log::{error, info};
use snafu::{ResultExt, Snafu};
use sp_core::crypto::AccountId32;
use sp_core::ByteArray;
use subxt::utils::H256;
use sxt_core::sxt_chain_runtime;
use sxt_core::sxt_chain_runtime::api::attestations::events::BlockAttested;
use sxt_core::sxt_chain_runtime::api::runtime_types::sxt_core::attestation::Attestation::EthereumAttestation;
use sxt_core::sxt_chain_runtime::api::runtime_types::sxt_core::attestation::EthereumSignature as RuntimeEthereumSignature;
use sxt_core::sxt_chain_runtime::api::staking::events::Unbonded;
use watcher::attestation;

use crate::chain_listener::{Block, BlockProcessor, API};

sol!(
    #[allow(clippy::too_many_arguments)]
    #[allow(missing_docs)]
    #[sol(rpc)]
    EventForwarder,
    "artifacts/EventForwarder.json"
);

/// Provider instance type for Ethereum transactions.
/// This handles gas estimation, nonce management, and wallet signing.
pub type ProviderInstance = alloy::providers::fillers::FillProvider<
    alloy::providers::fillers::JoinFill<
        alloy::providers::fillers::JoinFill<
            alloy::providers::Identity,
            alloy::providers::fillers::JoinFill<
                alloy::providers::fillers::GasFiller,
                alloy::providers::fillers::JoinFill<
                    alloy::providers::fillers::BlobGasFiller,
                    alloy::providers::fillers::JoinFill<
                        alloy::providers::fillers::NonceFiller,
                        alloy::providers::fillers::ChainIdFiller,
                    >,
                >,
            >,
        >,
        alloy::providers::fillers::WalletFiller<EthereumWallet>,
    >,
    alloy::providers::RootProvider,
    alloy::network::Ethereum,
>;

/// A processor that listens for blockchain events and interacts with the `EventForwarder` contract.
pub struct EventForwarderProcessor {
    provider: Arc<ProviderInstance>,
    address: Address,
}

impl EventForwarderProcessor {
    /// Creates a new `EventForwarderProcessor`.
    ///
    /// # Parameters
    /// - `provider`: Ethereum provider instance.
    /// - `address`: Address of the deployed `EventForwarder` contract.
    ///
    /// # Returns
    /// A new instance of `EventForwarderProcessor`.
    pub fn new(provider: Arc<ProviderInstance>, address: Address) -> Self {
        Self { provider, address }
    }

    /// Fetches attestation events from a given block.
    ///
    /// # Parameters
    /// - `block`: The block from which to fetch attestations.
    ///
    /// # Returns
    /// - `Ok(Vec<BlockAttested>)` if attestation events were successfully retrieved.
    /// - `Err(Error::FetchAttestation)` if an error occurs while fetching events.
    async fn fetch_block_attestations(&self, block: &Block) -> Result<Vec<BlockAttested>, Error> {
        let mut attestations = Vec::new();

        let events = block.events().await.context(FetchAttestationSnafu)?;
        for event in events.iter().flatten() {
            if let Ok(Some(attestation)) = event.as_event::<BlockAttested>() {
                attestations.push(attestation);
            }
        }

        Ok(attestations)
    }

    /// Processes attestation events and forwards staking-related data to the Ethereum contract.
    ///
    /// # Parameters
    /// - `api`: A reference to the blockchain API.
    /// - `attestations`: A list of attestation events to process.
    /// - `parent_block_hash`: The hash of the parent block.
    ///
    /// # Returns
    /// - `Ok(())` if processing is successful.
    /// - `Err(Error::BlockchainProcessing)` if an error occurs.
    pub async fn process_attestation(
        &self,
        api: &API,
        attestations: &[BlockAttested],
        parent_block_hash: H256,
    ) -> Result<(), Error> {
        let contract = EventForwarder::new(self.address, self.provider.clone());

        let attestation = attestations.first();
        if attestation.is_none() {
            info!("No attestations found for block");
            return Ok(());
        }
        let attestation = attestation.unwrap();

        let attested_block = Self::fetch_attested_block(api, attestation).await?;
        info!("Fetched attested block {}", attestation.block_number);

        // Fetch unbonding events
        let unbondings = Self::fetch_unbonding_events(&attested_block).await?;
        if unbondings.is_empty() {
            info!(
                "No unbonding events found in attested block {}",
                attestation.block_number
            );
        } else {
            info!(
                "Found {} unbonding event(s) in attested block {}",
                unbondings.len(),
                attestation.block_number
            );

            let (commitments, accounts) =
                attestation::fetch::commitments_and_accounts(api, attested_block.hash())
                    .await
                    .context(FetchCommitmentsAndAccountsSnafu)?;

            let tree = self
                .build_merkle_tree(commitments.clone(), accounts.clone())
                .await?;

            let state_root = tree
                .root
                .as_ref()
                .ok_or(Error::EmptyMerkleRoot)?
                .data
                .clone();
            let state_root =
                hex::decode(state_root.clone()).context(DecodeStateRootSnafu { state_root })?;
            let state_root = FixedBytes::<32>::from_slice(&state_root);

            // Collect signature components into arrays
            let mut r_values: Vec<FixedBytes<32>> = Vec::new();
            let mut s_values: Vec<FixedBytes<32>> = Vec::new();
            let mut v_values: Vec<u8> = Vec::new();
            let mut expected_addresses: Vec<Address> = Vec::new();

            for attestation in attestations.iter() {
                let EthereumAttestation {
                    signature,
                    address20,
                    ..
                } = &attestation.attestation;

                let RuntimeEthereumSignature { r, s, v } = signature;

                let v = if *v == 0 { 27 } else { 28 };

                let r = FixedBytes::<32>::from_slice(r);
                let s = FixedBytes::<32>::from_slice(s);

                let address = Address::from_slice(&address20.0);

                // Append the extracted values into arrays
                r_values.push(r);
                s_values.push(s);
                v_values.push(v);
                expected_addresses.push(address);
            }

            // Process unstaking for each unbonded stash account
            for Unbonded { stash, .. } in unbondings.iter() {
                let balance_query = sxt_chain_runtime::api::storage().system().account(stash);
                let balance = api
                    .storage()
                    .at(attested_block.hash())
                    .fetch(&balance_query)
                    .await
                    .context(FetchBalanceSnafu {
                        account_id: stash.clone(),
                    })?
                    .ok_or(Error::NoBalanceError {
                        account_id: stash.clone(),
                    })?;
                let free_balance = balance.data.free;

                let account_id = sp_core::crypto::AccountId32::from_slice(&stash.0)
                    .expect("should always be convertible");

                let encoded_leaf = watcher::attestation::fetch::encode_account_leaf(
                    account_id.clone(),
                    free_balance,
                );
                let proof = self.generate_proof(&tree, &encoded_leaf)?;

                let account_id = FixedBytes::<32>::from_slice(account_id.as_slice());

                // Call processUnstake with collected arrays
                match contract
                    .processUnstake(
                        account_id,
                        free_balance,
                        attestation.block_number,
                        state_root,
                        proof,
                        r_values.clone(),
                        s_values.clone(),
                        v_values.clone(),
                        expected_addresses.clone(),
                        U256::from(1),
                    )
                    .send()
                    .await
                {
                    Ok(tx) => {
                        info!("processUnstake tx sent: {}", tx.tx_hash());
                    }
                    Err(e) => {
                        error!("Failed to send transaction: {}", e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Fetches the block that was attested.
    async fn fetch_attested_block(api: &API, attestation: &BlockAttested) -> Result<Block, Error> {
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

    /// Fetches unbonding events from a block.
    async fn fetch_unbonding_events(block: &Block) -> Result<Vec<Unbonded>, Error> {
        let mut unbondings = Vec::new();

        let events = block.events().await.context(FetchUnbondingSnafu)?;
        for event in events.iter().flatten() {
            if let Ok(Some(unbonding)) = event.as_event::<Unbonded>() {
                unbondings.push(unbonding);
            }
        }

        Ok(unbondings)
    }

    /// create the merkle tree
    async fn build_merkle_tree(
        &self,
        commitments: Vec<String>,
        accounts: Vec<String>,
    ) -> Result<attestation::merkle_tree::MerkleTree, Error> {
        let mut data: Vec<String> = Vec::new();
        data.extend(commitments);
        data.extend(accounts);

        let hashed_data = attestation::merkle_tree::hash_data(data).context(HashingDataSnafu)?;
        let tree = attestation::merkle_tree::build_merkle_tree(hashed_data)
            .context(ConstructingMerkleTreeSnafu)?;

        if tree.root.is_none() {
            return Err(Error::EmptyMerkleRoot);
        }

        Ok(tree)
    }

    /// generate a proof from the tree
    fn generate_proof(
        &self,
        tree: &attestation::merkle_tree::MerkleTree,
        account_hex: &str,
    ) -> Result<Vec<FixedBytes<32>>, Error> {
        let account_leaf = keccak256(account_hex).context(KeccakSnafu)?;
        let account_leaf = keccak256(&account_leaf).context(KeccakSnafu)?;

        let leaf_index = tree
            .locate_leaf(&account_leaf)
            .ok_or(Error::LocateLeafError)?;

        let proof = tree.generate_proof(leaf_index);

        convert_proof(proof).map_err(|_| Error::InvalidProofLength)
    }
}

#[async_trait]
impl BlockProcessor for EventForwarderProcessor {
    async fn process_block(&self, api: &API, block: Block) {
        info!("AttestationProcessor processing block: {}", block.number());

        // Fetch attestation events
        let attestations = match self.fetch_block_attestations(&block).await {
            Ok(attestations) if !attestations.is_empty() => attestations,
            Ok(_) => {
                info!("No attestation events found in block {}", block.number());
                return;
            }
            Err(e) => {
                error!("Failed to fetch attestation events: {}", e);
                return;
            }
        };

        info!(
            "Found {} attestation(s) in block {}",
            attestations.len(),
            block.number()
        );

        // Process each attestation
        if let Err(e) = self
            .process_attestation(api, &attestations, block.hash())
            .await
        {
            error!(
                "Failed to process attestation for block {}: {}",
                block.number(),
                e
            );
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

    /// Error hashing data for the Merkle tree.
    ///
    /// - **Cause:** A problem occurred during hashing operations using Keccak.
    /// - **Solution:** Ensure data being hashed is correctly formatted.
    #[snafu(display("Error hashing data: {source}"))]
    HashingData {
        /// source error
        source: attestation::merkle_tree::MerkleTreeError,
    },

    /// Error constructing the Merkle tree.
    ///
    /// - **Cause:** The input data might be invalid or malformed.
    /// - **Solution:** Ensure the commitments and accounts are correctly formatted.
    #[snafu(display("Error constructing Merkle tree: {source}"))]
    ConstructingMerkleTree {
        /// source error
        source: attestation::merkle_tree::MerkleTreeError,
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
}

fn convert_proof(
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
