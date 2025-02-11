#![allow(warnings)]

use std::sync::Arc;

use alloy::network::{EthereumWallet, TransactionBuilder};
use alloy::node_bindings::{Anvil, AnvilInstance};
use alloy::primitives::{Address, Bytes, FixedBytes, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::TransactionRequest;
use alloy::signers::local::PrivateKeySigner;
use alloy::sol;
use async_trait::async_trait;
use codec::{Decode, Encode};
use eth_merkle_tree::utils::keccak::keccak256;
use log::{error, info};
use proof_of_sql_commitment_map::CommitmentScheme;
use serde_json::json;
use snafu::{ResultExt, Snafu};
use subxt::utils::H256;
use subxt::{OnlineClient, PolkadotConfig};
use sxt_core::attestation::EthereumSignature;
use sxt_core::sxt_chain_runtime::api::attestations::calls::types::attest_block::Attestation;
use sxt_core::sxt_chain_runtime::api::attestations::events::BlockAttested;
use sxt_core::sxt_chain_runtime::api::runtime_types::sxt_core::attestation::Attestation::EthereumAttestation;
use sxt_core::sxt_chain_runtime::api::staking::events::Unbonded;
use sxt_core::tables::{TableIdentifier, TableName, TableNamespace};
use sxt_core::ByteString;
use watcher::attestation;

use crate::chain_listener::{Block, BlockProcessor, API};

/// Enum representing errors that can occur in attestation processing.
#[derive(Debug, Snafu)]
pub enum KitchenSinkProcessorError {
    /// Error fetching attestation events from a block.
    #[snafu(display("Failed to fetch attestation events: {source}"))]
    FetchAttestation {
        /// todo
        source: subxt::Error,
    },

    /// Error fetching the attested block.
    #[snafu(display("Failed to fetch attested block {}: {source}", block_number))]
    FetchAttestedBlock {
        /// todo
        block_number: u32,
        /// todo
        source: subxt::Error,
    },

    /// Error fetching unbonding events from an attested block.
    #[snafu(display("Failed to fetch unbonding events: {source}"))]
    FetchUnbonding {
        /// todo
        source: subxt::Error,
    },

    /// Error calling `chain_getBlockHash` for an attested block.
    #[snafu(display("Failed to get block hash for block {}: {source}", block_number))]
    GetBlockHash {
        /// todo
        block_number: u32,
        /// todo
        source: subxt::Error,
    },

    /// Error serializing parameters for RPC calls.
    #[snafu(display("Failed to serialize parameters: {source}"))]
    Serialization {
        /// todo
        source: serde_json::Error,
    },

    /// Error fetching commitments and accounts.
    #[snafu(display("Error fetching commitments and accounts: {source}"))]
    FetchCommitmentsAndAccounts {
        /// todo
        source: attestation::fetch::FetchError,
    },

    /// Error hashing data for the Merkle tree.
    #[snafu(display("Error hashing data: {source}"))]
    HashingData {
        /// todo
        source: attestation::merkle_tree::MerkleTreeError,
    },

    /// Error constructing the Merkle tree.
    #[snafu(display("Error constructing Merkle tree: {source}"))]
    ConstructingMerkleTree {
        /// todo
        source: attestation::merkle_tree::MerkleTreeError,
    },

    /// Merkle tree generated an empty state root.
    #[snafu(display("Merkle tree calculated an empty state root"))]
    EmptyMerkleRoot,

    /// Failed to decode hex data.
    #[snafu(display("Failed to decode hex string: {source}"))]
    HexDecoding {
        /// todo
        source: hex::FromHexError,
    },

    /// Proof format is invalid (e.g., incorrect length).
    #[snafu(display("Invalid proof length"))]
    InvalidProofLength,

    /// Failed to locate the leaf in the Merkle tree.
    #[snafu(display("Error locating leaf in Merkle tree"))]
    LocateLeafError,

    /// Error generating a proof from the Merkle tree.
    #[snafu(display("Error generating proof from Merkle tree"))]
    GenerateProofError,

    /// Transaction failed to send.
    #[snafu(display("Transaction failed to send: {source}"))]
    TransactionSend {
        /// todo
        source: alloy::contract::Error,
    },

    /// Transaction confirmation failed.
    #[snafu(display("Transaction confirmation failed: {source}"))]
    TransactionConfirm {
        /// todo
        source: alloy::contract::Error,
    },

    /// todo
    #[snafu(display("Could not get commitment"))]
    MissingCommitment,

    /// todo
    #[snafu(display("Failed to decode TableIdentifier: {source}"))]
    TableIdentifierDecode {
        /// todo
        source: codec::Error,
    },

    /// todo
    #[snafu(display("Failed to decode CommitmentScheme: {source}"))]
    CommitmentSchemeDecode {
        /// todo
        source: codec::Error,
    },

    /// todo
    #[snafu(display("Commitment data missing or malformed"))]
    CommitmentDataError,

    /// todo
    #[snafu(display("Failed to decode TableName: {source}"))]
    TableNameDecode {
        /// todo
        source: codec::Error,
    },

    /// todo
    #[snafu(display("Failed to decode TableNamespace: {source}"))]
    TableNamespaceDecode {
        /// todo
        source: codec::Error,
    },
}

/// A processor that handles attestation and unbonding events.
pub struct KitchenSinkProcessor {
    anvil: Option<AnvilInstance>,
    provider: Arc<ProviderInstance>,
    address: Address,
}

impl KitchenSinkProcessor {
    /// todo
    pub async fn from_existing_deployment(
        provider: Arc<ProviderInstance>,
        address: Address,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            provider,
            address,
            anvil: None,
        })
    }
    // pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
    //     let anvil = Anvil::new().block_time(1).try_spawn()?;
    //     info!("Started anvil at {}", anvil.endpoint_url());

    //     // Set up signer from the first default Anvil account (Alice).
    //     let signer: PrivateKeySigner = anvil.keys()[0].clone().into();
    //     let wallet = EthereumWallet::from(signer.clone());

    //     // Set up the HTTP provider with the `reqwest` crate.
    //     let rpc_url = anvil.endpoint_url();
    //     let provider: Arc<ProviderInstance> =
    //         Arc::new(ProviderBuilder::new().wallet(wallet).on_http(rpc_url));

    //     let bytes_32 = FixedBytes::<32>::from([0x00; 32]); // 32 bytes filled with 0xAA

    //     let contract = Verifier::deploy(&provider).await?;

    //     info!(
    //         "Deployed verification contract at address: {}",
    //         contract.address()
    //     );

    //     Ok(Self {
    //         anvil: Some(anvil),
    //         provider: provider.clone(),
    //         address: *contract.address(),
    //     })
    // }

    /// todo
    pub async fn process_attestation(
        &self,
        api: &API,
        attestations: &[BlockAttested],
        parent_block_hash: H256,
    ) -> Result<(), KitchenSinkProcessorError> {
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
        }

        let (commitments, accounts) =
            attestation::fetch::commitments_and_accounts(api, attested_block.hash())
                .await
                .context(FetchCommitmentsAndAccountsSnafu)?;

        let tree = self
            .build_merkle_tree(commitments.clone(), accounts.clone())
            .await?;

        let contract = Arc::new(Verifier::new(self.address, self.provider.clone()));
        let first_account = accounts
            .first()
            .ok_or_else(|| KitchenSinkProcessorError::InvalidProofLength)?;

        let (account_id, balance) = self.extract_account_data(first_account)?;

        let proof = self.generate_proof(&tree, first_account)?;

        let state_root = hex::decode(tree.root.as_ref().unwrap().data.clone())
            .expect("could not decode state root");
        let state_root = FixedBytes::<32>::from_slice(state_root.as_slice());

        // Start check to ensure we have calculated the state root correctly
        let calculated_state_root = state_root;
        let EthereumAttestation {
            signature,
            proposed_pub_key,
            address20,
            state_root,
            block_number,
            block_hash,
        } = &attestation.attestation;

        let calculated_state_root =
            hex::decode(tree.root.expect("could not get root").data).expect("could not decode sr");
        let attested_state_root = state_root.0.clone();
        assert_eq!(calculated_state_root, attested_state_root);
        // end

        // start check of signatures on state root
        let state_root = FixedBytes::<32>::from_slice(&calculated_state_root);
        let address = Address::from_slice(&address20.0);

        let sxt_core::sxt_chain_runtime::api::runtime_types::sxt_core::attestation::EthereumSignature { r, s, v } = signature;

        let mut v = *v;

        if v == 0 {
            v = 27;
        } else {
            v = 28;
        }

        let r = FixedBytes::<32>::from_slice(r);
        let r = vec![r];

        let s = FixedBytes::<32>::from_slice(s);
        let s = vec![s];

        let v = vec![v];
        let expected_addresses = vec![address];

        match contract
            .processUnstake(
                account_id,
                balance,
                *block_number,
                state_root,
                proof,
                r,
                s,
                v,
                expected_addresses,
                U256::from(1),
            )
            .send()
            .await
        {
            Ok(tx) => {
                info!("Transaction sent: {:?}", tx.tx_hash());
            }
            Err(e) => {
                error!("Failed to send transaction: {}", e);
            }
        }

        Ok(())
    }

    /// Fetches the block that was attested.
    async fn fetch_attested_block(
        api: &API,
        attestation: &BlockAttested,
    ) -> Result<Block, KitchenSinkProcessorError> {
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
    async fn fetch_unbonding_events(
        block: &Block,
    ) -> Result<Vec<Unbonded>, KitchenSinkProcessorError> {
        let mut unbondings = Vec::new();

        let events = block.events().await.context(FetchUnbondingSnafu)?;
        for event in events.iter().flatten() {
            if let Ok(Some(unbonding)) = event.as_event::<Unbonded>() {
                unbondings.push(unbonding);
            }
        }

        Ok(unbondings)
    }

    /// todo
    async fn build_merkle_tree(
        &self,
        commitments: Vec<String>,
        accounts: Vec<String>,
    ) -> Result<attestation::merkle_tree::MerkleTree, KitchenSinkProcessorError> {
        let mut data: Vec<String> = Vec::new();
        data.extend(commitments);
        data.extend(accounts);

        let hashed_data = attestation::merkle_tree::hash_data(data).context(HashingDataSnafu)?;
        let tree = attestation::merkle_tree::build_merkle_tree(hashed_data)
            .context(ConstructingMerkleTreeSnafu)?;

        if tree.root.is_none() {
            return Err(KitchenSinkProcessorError::EmptyMerkleRoot);
        }

        Ok(tree)
    }

    /// todo
    fn extract_account_data(
        &self,
        account_hex: &String,
    ) -> Result<(FixedBytes<32>, u128), KitchenSinkProcessorError> {
        let decoded_bytes = hex::decode(account_hex).context(HexDecodingSnafu)?;

        if decoded_bytes.len() != 48 {
            return Err(KitchenSinkProcessorError::InvalidProofLength);
        }

        let account_id_bytes: [u8; 32] = decoded_bytes[0..32]
            .try_into()
            .map_err(|_| KitchenSinkProcessorError::InvalidProofLength)?;

        let balance_bytes: [u8; 16] = decoded_bytes[32..48]
            .try_into()
            .map_err(|_| KitchenSinkProcessorError::InvalidProofLength)?;

        let balance = u128::from_be_bytes(balance_bytes);

        Ok((FixedBytes::<32>::from(account_id_bytes), balance))
    }

    /// todo
    fn generate_proof(
        &self,
        tree: &attestation::merkle_tree::MerkleTree,
        account_hex: &str,
    ) -> Result<Vec<FixedBytes<32>>, KitchenSinkProcessorError> {
        let account_leaf =
            keccak256(account_hex).map_err(|_| KitchenSinkProcessorError::LocateLeafError)?;
        let account_leaf =
            keccak256(&account_leaf).map_err(|_| KitchenSinkProcessorError::LocateLeafError)?;

        let leaf_index = tree
            .locate_leaf(&account_leaf)
            .ok_or(KitchenSinkProcessorError::LocateLeafError)?;

        let proof = tree.generate_proof(leaf_index);

        convert_proof(proof).map_err(|_| KitchenSinkProcessorError::InvalidProofLength)
    }

    /// todo
    async fn send_verification(
        &self,
        contract: VerifierContract<'static>,
        proof: Vec<FixedBytes<32>>,
        account_id: FixedBytes<32>,
        balance: u128,
        state_root_hex: String,
    ) -> Result<(), KitchenSinkProcessorError> {
        let state_root = hex::decode(state_root_hex).context(HexDecodingSnafu)?;
        let state_root = FixedBytes::<32>::from_slice(state_root.as_slice());

        match contract
            .verifyAccountProof(state_root, proof, account_id, balance)
            .send()
            .await
        {
            Ok(tx) => {
                info!("Transaction sent: {:?}", tx.tx_hash());

                match tx.with_required_confirmations(3).watch().await {
                    Ok(receipt) => {
                        info!("Transaction confirmed! Receipt: {:?}", receipt);
                    }
                    Err(e) => {
                        error!("Transaction confirmation failed: {}", e);
                        return Err(KitchenSinkProcessorError::TransactionConfirm {
                            source: alloy::contract::Error::PendingTransactionError(e),
                        });
                    }
                }
            }
            Err(e) => {
                error!("Failed to send transaction: {}", e);
                return Err(KitchenSinkProcessorError::TransactionSend { source: e });
            }
        }

        Ok(())
    }
}

#[async_trait]
impl BlockProcessor for KitchenSinkProcessor {
    /// todo
    async fn process_block(&self, api: &API, block: Block) {
        info!("AttestationProcessor processing block: {}", block.number());

        // Fetch attestation events
        let attestations = match fetch_block_attestations(&block).await {
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

/// Converts a `Vec<String>` containing hex-encoded 32-byte values into `Vec<FixedBytes<32>>`
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

/// todo
pub fn decode_commitment_data(
    commitment_hex: &str,
) -> Result<(TableName, TableNamespace, CommitmentScheme, Vec<u8>), KitchenSinkProcessorError> {
    // Convert hex string to bytes
    let decoded_bytes = hex::decode(commitment_hex).context(HexDecodingSnafu)?;

    // Convert to a readable input slice
    let mut input = &decoded_bytes[..];

    let name = ByteString::decode(&mut input).context(TableNameDecodeSnafu)?;

    let namespace = ByteString::decode(&mut input).context(TableNamespaceDecodeSnafu)?;

    // Decode CommitmentScheme
    let commitment_scheme =
        CommitmentScheme::decode(&mut input).context(CommitmentSchemeDecodeSnafu)?;

    // Remaining bytes are the commitment
    let commitment = input.to_vec();
    if commitment.is_empty() {
        return Err(KitchenSinkProcessorError::CommitmentDataError);
    }

    Ok((name, namespace, commitment_scheme, commitment))
}

/// todo
async fn fetch_block_attestations(
    block: &Block,
) -> Result<Vec<BlockAttested>, KitchenSinkProcessorError> {
    let mut attestations = Vec::new();

    let events = block.events().await.context(FetchAttestationSnafu)?;
    for event in events.iter().flatten() {
        if let Ok(Some(attestation)) = event.as_event::<BlockAttested>() {
            attestations.push(attestation);
        }
    }

    Ok(attestations)
}

sol!(
    #[allow(clippy::too_many_arguments)]
    #[allow(missing_docs)]
    #[sol(rpc)]
    Verifier,
    "artifacts/EventForwarder.json"
);

/// todo
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

/// todo
pub type VerifierContract<'a> = Verifier::VerifierInstance<
    (),
    &'a alloy::providers::fillers::FillProvider<
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
>;
