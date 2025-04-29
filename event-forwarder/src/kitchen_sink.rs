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
use attestation_tree::{
    attestation_tree_from_prefixes,
    decode_storage_key_and_value,
    prove_leaf_pair,
    AttestationTreeError,
    AttestationTreeProofError,
    DecodeStorageError,
    LocksStakingPrefixFoliate,
    STAKING_BALANCE_LOCK_ID,
};
use codec::{Decode, Encode};
use eth_merkle_tree::utils::keccak::keccak256;
use log::{error, info};
use proof_of_sql_commitment_map::CommitmentScheme;
use serde_json::json;
use snafu::{ResultExt, Snafu};
use sp_core::crypto::AccountId32;
use subxt::utils::H256;
use subxt::{OnlineClient, PolkadotConfig};
use subxt_signer::sr25519::Keypair;
use sxt_core::attestation::EthereumSignature;
use sxt_core::sxt_chain_runtime::api::attestations::calls::types::attest_block::Attestation;
use sxt_core::sxt_chain_runtime::api::attestations::events::BlockAttested;
use sxt_core::sxt_chain_runtime::api::runtime_types::sxt_core::attestation::Attestation::EthereumAttestation;
use sxt_core::sxt_chain_runtime::api::staking::events::Unbonded;
use sxt_core::tables::{TableIdentifier, TableName, TableNamespace};
use sxt_core::ByteString;
use sxt_runtime::Runtime;
use tokio::sync::mpsc;
use watcher::attestation;

use crate::block_processing;
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

    /// Error fetching commitments and locks.
    #[snafu(display("Error fetching commitments and locks: {source}"))]
    FetchCommitmentsAndAccounts {
        /// todo
        source: attestation::fetch::FetchError,
    },

    /// Error decoding fetched locks storage bytes.
    #[snafu(display("Error decoding fetched locks storage bytes: {source}"))]
    DecodeStorageLocks {
        /// The source decode .
        source: DecodeStorageError,
    },

    /// Error constructing the Merkle tree.
    #[snafu(display("Error constructing Merkle tree: {source}"))]
    ConstructingMerkleTree {
        /// The source attestation tree error.
        source: AttestationTreeError,
    },

    /// Error proving that attestation tree contains the claimed balance locks.
    #[snafu(display(
        "Encountered error when proving that merkle tree contains claimed balance locks: {source}"
    ))]
    LocksProof {
        /// The source attestation tree proof error.
        source: AttestationTreeProofError,
    },

    /// Error getting the staking locks for the given account.
    #[snafu(display(
        "Error retrieving the locked staking balance for the given account: {account_id}"
    ))]
    NoStakingLocks {
        ///  The account id without any staking locks.
        account_id: AccountId32,
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

    /// Error originating from the block processing module
    #[snafu(display("BlockProcessingError: {source}"))]
    BlockProcessingError {
        /// Source of the error
        source: block_processing::Error,
    },
}

/// A processor that handles attestation and unbonding events.
pub struct KitchenSinkProcessor {
    anvil: Option<AnvilInstance>,
    provider: Arc<ProviderInstance>,
    address: Address,
    channel: Option<mpsc::Sender<bool>>,
    keypair: Keypair,
    initial_nonce: u64,
}

impl KitchenSinkProcessor {
    /// todo
    pub async fn from_existing_deployment(
        provider: Arc<ProviderInstance>,
        address: Address,
        channel: Option<mpsc::Sender<bool>>,
        keypair: Keypair,
        initial_nonce: u64,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            provider,
            address,
            anvil: None,
            channel,
            keypair,
            initial_nonce,
        })
    }

    /// todo
    pub async fn process_attestation(
        &mut self,
        api: &API,
        attestations: &[BlockAttested],
        parent_block_hash: H256,
        attested_block_number: u32,
    ) -> Result<(), KitchenSinkProcessorError> {
        let attestation = attestations.first();
        if attestation.is_none() {
            info!("No attestations found for block");
            let new_nonce = block_processing::mark_block_forwarded(
                api,
                attested_block_number,
                &self.keypair,
                self.initial_nonce,
            )
            .await
            .context(BlockProcessingSnafu)?;
            self.initial_nonce = new_nonce;
            block_processing::send_channel_update(&self.channel).await;
            return Ok(());
        }
        let attestation = attestation.unwrap();

        let attested_block = block_processing::fetch_attested_block(api, attestation)
            .await
            .context(BlockProcessingSnafu)?;
        info!("Fetched attested block {}", attestation.block_number);

        // Fetch unbonding events
        let unbondings = block_processing::fetch_unbonding_events(&attested_block)
            .await
            .context(BlockProcessingSnafu)?;

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

        let (commitments, locks, staking_contract_info_bytes) =
            attestation::fetch::commitments_and_locks_and_staking_contract_info(
                api,
                attested_block.hash(),
            )
            .await
            .context(FetchCommitmentsAndAccountsSnafu)?;

        let (first_locks_key_bytes, first_locks_data_bytes) = locks
            .first()
            .ok_or_else(|| KitchenSinkProcessorError::InvalidProofLength)?;

        let (first_locks_key, (first_locks_data, staking_contract_info)) =
            decode_storage_key_and_value::<LocksStakingPrefixFoliate<Runtime>>(
                first_locks_key_bytes,
                first_locks_data_bytes
                    .iter()
                    .copied()
                    .chain(staking_contract_info_bytes.clone())
                    .collect::<Vec<_>>()
                    .as_slice(),
            )
            .context(DecodeStorageLocksSnafu)?;

        let tree = attestation_tree_from_prefixes::<_, _, Runtime>(
            commitments,
            locks,
            staking_contract_info_bytes,
        )
        .context(ConstructingMerkleTreeSnafu)?;

        let contract = Arc::new(Verifier::new(self.address, self.provider.clone()));

        let proof = prove_leaf_pair::<LocksStakingPrefixFoliate<Runtime>>(
            &tree,
            first_locks_key.clone(),
            (first_locks_data.clone(), staking_contract_info),
        )
        .context(LocksProofSnafu)?;

        let proof = block_processing::convert_proof(proof)
            .map_err(|_| KitchenSinkProcessorError::InvalidProofLength)?;

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

        let balance = first_locks_data
            .iter()
            .find(|balance_lock| balance_lock.id == *STAKING_BALANCE_LOCK_ID)
            .ok_or(KitchenSinkProcessorError::NoStakingLocks {
                account_id: first_locks_key.0.clone(),
            })?
            .amount;

        match contract
            .processUnstake(
                FixedBytes(first_locks_key.0.into()),
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

        let new_nonce = block_processing::mark_block_forwarded(
            api,
            attested_block_number,
            &self.keypair,
            self.initial_nonce,
        )
        .await
        .context(BlockProcessingSnafu)?;
        self.initial_nonce = new_nonce;
        block_processing::send_channel_update(&self.channel).await;

        Ok(())
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
    async fn process_block(&mut self, api: &API, block: Block) {
        info!("AttestationProcessor processing block: {}", block.number());

        // Fetch attestation events
        let attestations = match fetch_block_attestations(&block).await {
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
            .process_attestation(api, &attestations, block.hash(), block.number())
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
    "artifacts/OldEventForwarder.json"
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
