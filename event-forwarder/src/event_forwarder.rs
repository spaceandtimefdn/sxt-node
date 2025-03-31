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
use alloy::primitives::{Address, FixedBytes, Uint};
use alloy::sol;
use async_trait::async_trait;
use attestation_tree::{
    attestation_tree_from_prefixes,
    prove_leaf_pair,
    AccountPrefixFoliate,
    AttestationTreeError,
    AttestationTreeProofError,
};
use codec::{Decode, Encode};
use eth_merkle_tree::tree::MerkleTree;
use frame_system::AccountInfo;
use log::{error, info};
use snafu::{ResultExt, Snafu};
use sp_core::crypto::AccountId32;
use sp_core::ByteArray;
use subxt_signer::sr25519::Keypair;
use sxt_core::sxt_chain_runtime;
use sxt_core::sxt_chain_runtime::api::attestations::events::BlockAttested;
use sxt_core::sxt_chain_runtime::api::runtime_types::sxt_core::attestation::Attestation::EthereumAttestation;
use sxt_core::sxt_chain_runtime::api::runtime_types::sxt_core::attestation::EthereumSignature as RuntimeEthereumSignature;
use sxt_core::sxt_chain_runtime::api::staking::events::Unbonded;
use sxt_runtime::Runtime;
use tokio::sync::mpsc;
use watcher::attestation;

use crate::block_processing;
use crate::chain_listener::{Block, BlockProcessor, API};

sol!(
    #[allow(clippy::too_many_arguments)]
    #[allow(missing_docs)]
    #[sol(rpc)]
    EventForwarder,
    "artifacts/0xd27Da90dfaabE287B572919A6f0aeEBc79a2Ed7e.json"
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

/// The concrete type of the event forwarder contract with default fillers
pub type EventForwarderInstance = EventForwarder::EventForwarderInstance<
    (),
    Arc<
        alloy::providers::fillers::FillProvider<
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
        >,
    >,
>;

/// A processor that listens for blockchain events and interacts with the `EventForwarder` contract.
pub struct EventForwarderProcessor {
    provider: Arc<ProviderInstance>,
    address: Address,
    keypair: Keypair,
    channel: Option<mpsc::Sender<bool>>,
    initial_nonce: u64,
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
    pub fn new(
        provider: Arc<ProviderInstance>,
        address: Address,
        keypair: Keypair,
        channel: Option<mpsc::Sender<bool>>,
        initial_nonce: u64,
    ) -> Self {
        Self {
            provider,
            address,
            keypair,
            channel,
            initial_nonce,
        }
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
        &mut self,
        api: &API,
        attestations: &[BlockAttested],
        attested_block_number: u32,
    ) -> Result<(), Error> {
        let contract = EventForwarder::new(self.address, self.provider.clone());

        let Some(attestation) = attestations.first() else {
            info!("No attestations found for this block");
            self.update_progress(api, attested_block_number).await?;
            return Ok(());
        };

        let attested_block = block_processing::fetch_attested_block(api, attestation)
            .await
            .context(BlockProcessingSnafu)?;
        info!("Fetched attested block {}", attestation.block_number);

        let unbondings = block_processing::fetch_unbonding_events(&attested_block)
            .await
            .context(BlockProcessingSnafu)?;

        if unbondings.is_empty() {
            info!(
                "No unbonding events found in attested block {}",
                attestation.block_number
            );
        } else {
            process_unbondings(api, &contract, attestations, &unbondings, &attested_block).await?;
        }

        self.update_progress(api, attested_block_number).await?;

        Ok(())
    }

    async fn update_progress(&mut self, api: &API, block_number: u32) -> Result<(), Error> {
        let new_nonce = block_processing::mark_block_forwarded(
            api,
            block_number,
            &self.keypair,
            self.initial_nonce,
        )
        .await
        .context(BlockProcessingSnafu)?;
        self.initial_nonce = new_nonce;
        block_processing::send_channel_update(&self.channel).await;
        Ok(())
    }
}

#[async_trait]
impl BlockProcessor for EventForwarderProcessor {
    async fn process_block(&mut self, api: &API, block: Block) {
        info!("AttestationProcessor processing block: {}", block.number());

        // Fetch attestation events
        let attestations = match block_processing::fetch_block_attestations(&block).await {
            Ok(attestations) => attestations,
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
            .process_attestation(api, &attestations, block.number())
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

async fn process_unbondings(
    api: &API,
    contract: &EventForwarderInstance,
    attestations: &[BlockAttested],
    unbondings: &[Unbonded],
    attested_block: &Block,
) -> Result<(), Error> {
    let first_attestation = attestations.first().unwrap();

    info!(
        "Found {} unbonding event(s) in attested block {}",
        unbondings.len(),
        first_attestation.block_number
    );

    let (commitments, accounts) =
        attestation::fetch::commitments_and_accounts(api, attested_block.hash())
            .await
            .context(FetchCommitmentsAndAccountsSnafu)?;

    let tree = attestation_tree_from_prefixes::<_, _, Runtime>(commitments, accounts)
        .context(ConstructingMerkleTreeSnafu)?;

    let state_root = extract_state_root(&tree)?;

    let (r_values, s_values, v_values, expected_addresses) = extract_signature_data(attestations);

    for Unbonded { stash, .. } in unbondings.iter() {
        if stash.0.as_ref()[..12] == [0; 12] {
            process_unstake(
                api,
                contract,
                first_attestation,
                stash,
                &tree,
                &state_root,
                &r_values,
                &s_values,
                &v_values,
                &expected_addresses,
                attested_block,
            )
            .await?;
        }
    }

    Ok(())
}

fn extract_state_root(tree: &MerkleTree) -> Result<FixedBytes<32>, Error> {
    let state_root = tree
        .root
        .as_ref()
        .ok_or(Error::EmptyMerkleRoot)?
        .data
        .clone();

    let decoded_root =
        hex::decode(state_root.clone()).context(DecodeStateRootSnafu { state_root })?;

    Ok(FixedBytes::<32>::from_slice(&decoded_root))
}

fn extract_signature_data(
    attestations: &[BlockAttested],
) -> (
    Vec<FixedBytes<32>>,
    Vec<FixedBytes<32>>,
    Vec<u8>,
    Vec<Address>,
) {
    let mut r_values = Vec::new();
    let mut s_values = Vec::new();
    let mut v_values = Vec::new();
    let mut expected_addresses = Vec::new();

    for attestation in attestations.iter() {
        let EthereumAttestation {
            signature,
            address20,
            ..
        } = &attestation.attestation;

        let RuntimeEthereumSignature { r, s, v } = signature;
        let v = if *v == 0 { 27 } else { 28 };

        r_values.push(FixedBytes::<32>::from_slice(r));
        s_values.push(FixedBytes::<32>::from_slice(s));
        v_values.push(v);
        expected_addresses.push(Address::from_slice(&address20.0));
    }

    (r_values, s_values, v_values, expected_addresses)
}

#[allow(clippy::too_many_arguments)]
/// Will likely get changed once interface becomes more concrete and we can start using solidity structs
async fn process_unstake(
    api: &API,
    contract: &EventForwarderInstance,
    attestation: &BlockAttested,
    stash: &subxt::utils::AccountId32,
    tree: &MerkleTree,
    state_root: &FixedBytes<32>,
    r_values: &[FixedBytes<32>],
    s_values: &[FixedBytes<32>],
    v_values: &[u8],
    expected_addresses: &[Address],
    attested_block: &Block,
) -> Result<(), Error> {
    let balance_query = sxt_chain_runtime::api::storage().system().account(stash);
    let account_info = api
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

    let staker = Address::from_slice(&stash.0.as_ref()[12..32]);
    let stash = sp_core::crypto::AccountId32::from_slice(&stash.0).expect("should always work");
    let account_info = AccountInfo::<
        <Runtime as frame_system::Config>::Nonce,
        <Runtime as frame_system::Config>::AccountData,
    >::decode(&mut account_info.encode().as_slice())
    .expect("should always work");
    let amount = Uint::from(account_info.data.free);

    let proof = prove_leaf_pair::<AccountPrefixFoliate<Runtime>>(tree, (stash,), account_info)
        .context(AccountBalanceProofSnafu)?;
    let proof = block_processing::convert_proof(proof).map_err(|_| Error::InvalidProofLength)?;

    match contract
        .sxtFulfillUnstake(
            staker,
            amount,
            attestation.block_number.into(),
            proof,
            r_values.to_vec(),
            s_values.to_vec(),
            v_values.to_vec(),
        )
        .send()
        .await
    {
        Ok(tx) => info!("processUnstake tx sent: {}", tx.tx_hash()),
        Err(e) => error!("Failed to send transaction: {}", e),
    }

    Ok(())
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

    /// Error constructing the Merkle tree.
    ///
    /// - **Cause:** The input data might be invalid or malformed.
    /// - **Solution:** Ensure the commitments and accounts are correctly formatted.
    #[snafu(display("Error constructing Merkle tree: {source}"))]
    ConstructingMerkleTree {
        /// source error
        source: AttestationTreeError,
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

    /// Error proving that attestation tree contains the claimed account balance.
    #[snafu(display(
        "Encountered error when proving that merkle tree contains account balance: {source}"
    ))]
    AccountBalanceProof {
        /// The source attestation tree proof error.
        source: AttestationTreeProofError,
    },

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

    /// An error originating in the block_processing.rs module
    #[snafu(display("BlockProcessingError: {source}"))]
    BlockProcessingError {
        /// The source of the error
        source: block_processing::Error,
    },
}
