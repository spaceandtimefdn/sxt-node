//! todo
use std::fs::File;
use std::io::{self, Read};
use std::sync::atomic::Ordering;
use std::sync::Arc;

use ::sxt_core::attestation::sign_eth_message;
use attestation_tree::attestation_tree_from_prefixes;
use clap::{Parser, Subcommand};
use crossterm::event::{read, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use env_logger::Env;
use futures::StreamExt;
use hex::FromHex;
use k256::ecdsa::SigningKey;
use log::{error, info, warn};
use prometheus::core::{Atomic, AtomicU64};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, List, ListItem};
use ratatui::Terminal;
use runtime::api::runtime_types::sxt_core::attestation::Attestation;
use sha3::digest::generic_array::GenericArray;
use subxt::blocks::Block as BlockT;
use subxt::config::polkadot::PolkadotExtrinsicParamsBuilder as Params;
use subxt::config::substrate::{BlakeTwo256, SubstrateHeader};
use subxt::config::Header;
use subxt::utils::H256;
use subxt::{OnlineClient, PolkadotConfig};
use subxt_signer::sr25519::Keypair;
use sxt_core::attestation::{
    create_attestation_message,
    verify_eth_signature,
    EthereumSignature,
    RegisterExternalAddress,
};
use sxt_core::sxt_chain_runtime as runtime;
use sxt_core::sxt_chain_runtime::api::runtime_types::bounded_collections::bounded_vec::BoundedVec;
use sxt_runtime::Runtime;
use thiserror::Error;
use watcher::attestation;

type SxtConfig = PolkadotConfig;

/// A block in the SXT network
pub type SxtBlock = subxt::blocks::Block<PolkadotConfig, OnlineClient<SxtConfig>>;

/// A single event in the SXT network
pub type SxtEvent = subxt::events::EventDetails<PolkadotConfig>;

/// A group of events in the SXT networks
pub type SxtEvents = subxt::events::Events<PolkadotConfig>;

/// Errors that may occur during the attestation process.
#[derive(Error, Debug)]
pub enum AttestationError {
    /// Occurs when reading a file fails.
    ///
    /// Typically used for loading keys or other necessary files.
    #[error("Failed to read file: {0}")]
    FileReadError(#[from] io::Error),

    /// Indicates that a provided key has an invalid length.
    ///
    /// Expected key length is 32 bytes, commonly for Substrate SR25519 or Ethereum private keys.
    #[error("Invalid key length: expected 32 bytes")]
    InvalidKeyLength,

    /// Occurs when the creation of an SR25519 keypair fails.
    ///
    /// This could happen due to invalid key material or unexpected issues during key derivation.
    #[error("Failed to create SR25519 keypair")]
    KeypairCreationError,

    /// Indicates a failure to parse a `SigningKey` from raw bytes.
    ///
    /// Likely due to incorrect formatting or invalid key data.
    #[error("Failed to parse SigningKey from bytes")]
    SigningKeyParseError,

    /// Represents an error originating from the Subxt library.
    ///
    /// Used to handle issues such as connection errors, RPC failures, or storage fetch errors.
    #[error("Subxt error: {0}")]
    SubxtError(#[from] subxt::Error),

    /// Error during the creation of an attestation.
    ///
    /// This may occur if the signing process fails or if the attestation format is invalid.
    #[error("Attestation creation error")]
    AttestationCreationError,

    /// Occurs when the provided account ID cannot be parsed.
    ///
    /// This typically indicates a formatting or encoding issue with the account ID.
    #[error("AccountId error, could not parse provided account id")]
    AccountIdError,

    /// Indicates a failure to decode a Substrate key from hex.
    ///
    /// Commonly caused by invalid or malformed hexadecimal input.
    #[error("HexDecodeError: Error decoding the substrate key as hex")]
    HexDecodeError,

    /// Represents an error originating from the `sxt-core` attestation module.
    ///
    /// Used to propagate specific attestation-related errors from the core library.
    #[error("sxt-core AttestationError: {0}")]
    SxtCoreAttestationError(#[from] sxt_core::attestation::AttestationError),

    /// Occurs when a transaction submission fails.
    ///
    /// Includes the reason for failure as a string message.
    #[error("TransactionFailed: {0}")]
    TransactionFailed(String),

    /// Indicates an error decoding an Ethereum key from hex.
    ///
    /// This typically means the input does not conform to the expected hexadecimal format.
    #[error("EthereumHexDecodeError")]
    EthereumHexDecodeError,

    /// Error when the block hash for a given block number cannot be found.
    ///
    /// This may indicate the block number is invalid or outside the chain's finalized state.
    #[error("Block hash not found for requested block number")]
    BlockHashNotFoundError,

    /// Represents an error fetching attestations for a specific block.
    ///
    /// Includes the block number for which the error occurred.
    #[error("There was an error reading the attestations for block {0} on chain")]
    ErrorFetchingAttestations(u32),

    /// Indicates that a signature validation for a specific block failed.
    ///
    /// Includes the block number for which the validation failed.
    #[error("Error validating signature for block {0}")]
    InvalidSignature(u32),

    /// Error indicating that attestations for a block have inconsistent state roots.
    ///
    /// This means the attestations cannot be verified due to a mismatch in state root values.
    #[error("The attestations have different state roots, impossible to verify")]
    StateRootMismatch,

    /// Error fetching commitments and accounts from the chain.
    #[error("FetchError: {0}")]
    FetchError(#[from] attestation::fetch::FetchError),
}

/// Command-line arguments for the CLI
#[derive(Parser, Debug)]
#[command(
    name = "watcher",
    about = "Attests finalized blocks for the SxT Network"
)]
struct Cli {
    /// WebSocket URL to connect to the Substrate node
    #[arg(
        short,
        long,
        default_value = "ws://127.0.0.1:9944",
        env = "SXT_NODE_WEBSOCKET"
    )]
    websocket: String,

    /// Path to the Ethereum private key file
    #[arg(long, default_value = "./eth.key", env = "SXT_ATTESTOR_ETH_KEY")]
    eth_key_path: String,

    /// Path to the Substrate SR25519 key file
    #[arg(
        long,
        default_value = "./substrate.key",
        env = "SXT_ATTESTOR_SUBSTRATE_KEY"
    )]
    substrate_key_path: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Watch finalized blocks and attest the results
    /// Requires being successfully registered as an attestor
    Run {
        /// The number of blocks to process concurrently.
        #[arg(long, env, default_value = "10")]
        block_process_concurrency: usize,
    },

    /// Create the registration details to become an SxT network attestor
    Register,

    /// Verify the integrity of a block by verifying the onchain attestations
    Verify {
        /// block number to verify
        #[arg(short, long)]
        block_number: u32,
    },
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let args = Cli::parse();

    match args.command {
        Commands::Run {
            block_process_concurrency,
        } => {
            // Use an async block instead of .and_then() to handle async operations
            if let Err(err) = async {
                let client = AttestationClient::new(
                    &args.websocket,
                    &args.eth_key_path,
                    &args.substrate_key_path,
                    block_process_concurrency,
                )
                .await?;
                client.run().await
            }
            .await
            {
                error!("{:?}", err);
            }
        }
        Commands::Register => {
            if let Err(err) = register(&args.eth_key_path, &args.substrate_key_path).await {
                error!("{:?}", err);
            }
        }
        Commands::Verify { block_number } => {
            if let Err(err) = verify(block_number, &args.websocket).await {
                error!("{:?}", err);
            }
        }
    }
}

/// A client for handling block attestations in the SxT network.
///
/// This struct manages the connection to the blockchain, signing keys, and tracking nonces for transactions.
#[derive(Debug)]
struct AttestationClient {
    /// The WebSocket URL used to connect to the Substrate blockchain node.
    ///
    /// This URL is typically in the format `ws://127.0.0.1:9944` for local nodes or a remote URL for production.
    websocket: String,

    /// The file path to the Ethereum private key.
    ///
    /// This key is used to sign attestations for the SxT network.
    eth_key_path: String,

    /// The file path to the Substrate SR25519 private key.
    ///
    /// This key is used to submit transactions to the blockchain.
    substrate_key_path: String,

    /// The Substrate API client used to interact with the blockchain.
    ///
    /// This client provides access to blocks, storage, and transaction submission.
    api: OnlineClient<SxtConfig>,

    /// The current transaction nonce for the associated account.
    ///
    /// This is used to ensure that transactions are sequentially ordered and prevent replay attacks.
    /// Wrapped in an `Arc` for shared access and updated atomically.
    nonce: Arc<AtomicU64>,

    /// The number of blocks to process concurrently.
    block_process_concurrency: usize,
}

impl AttestationClient {
    async fn new(
        websocket: &str,
        eth_key_path: &str,
        substrate_key_path: &str,
        block_process_concurrency: usize,
    ) -> Result<Self, AttestationError> {
        let api = OnlineClient::<PolkadotConfig>::from_insecure_url(websocket).await?;

        info!("Connected to chain at {}", websocket);

        let initial_nonce =
            AttestationClient::fetch_initial_nonce(&api, substrate_key_path).await?;
        let nonce = Arc::new(AtomicU64::new(initial_nonce));

        Ok(Self {
            websocket: websocket.to_string(),
            eth_key_path: eth_key_path.to_string(),
            substrate_key_path: substrate_key_path.to_string(),
            api,
            nonce,
            block_process_concurrency,
        })
    }

    async fn fetch_initial_nonce(
        api: &OnlineClient<SxtConfig>,
        substrate_key_path: &str,
    ) -> Result<u64, AttestationError> {
        let substrate_key = load_substrate_key(substrate_key_path)?;
        let account_id = substrate_key.public_key();
        let addr = runtime::api::storage()
            .system()
            .account(account_id.to_account_id());

        let account_info = api.storage().at_latest().await?.fetch(&addr).await?;

        Ok(account_info.map_or(0, |info| info.nonce as u64))
    }

    async fn run(&self) -> Result<(), AttestationError> {
        let eth_signing_key = load_ethereum_key(&self.eth_key_path)?;
        let substrate_key = load_substrate_key(&self.substrate_key_path)?;

        self.api
            .blocks()
            .subscribe_finalized()
            .await?
            .for_each_concurrent(self.block_process_concurrency, |block_result| async {
                let _ = self
                    .process_block(block_result, &eth_signing_key, &substrate_key)
                    .await;
            })
            .await;

        Ok(())
    }

    async fn process_block(
        &self,
        block_result: Result<SxtBlock, subxt::Error>,
        private_key: &SigningKey,
        keypair: &Keypair,
    ) -> Result<(), ()> {
        let block = match block_result {
            Ok(block) => block,
            Err(e) => {
                log::error!("Error retrieving block: {}", e);
                return Ok(()); // Swallow the error and continue
            }
        };

        info!("Processing block {:?}", block.number());

        let (commitments, locks, contract_info) =
            match attestation::fetch::commitments_and_locks_and_staking_contract_info(
                &self.api,
                block.hash(),
            )
            .await
            {
                Ok(result) => result,
                Err(e) => {
                    log::error!("Error fetching commitments and accounts: {}", e);
                    return Ok(());
                }
            };

        let tree = match attestation_tree_from_prefixes::<_, _, Runtime>(
            commitments,
            locks,
            contract_info,
        ) {
            Ok(result) => result,
            Err(e) => {
                log::error!("Error creating attestation tree: {}", e);
                return Ok(());
            }
        };

        let state_root = match tree.root {
            Some(root) => root,
            None => {
                log::error!("Error: the tree calculated an empty state root");
                return Ok(());
            }
        };

        let hex_decoded_state_root =
            hex::decode(state_root.data.clone()).expect("could not decode for msg creation");

        let message = create_attestation_message(&hex_decoded_state_root, block.number());

        let signature = match generate_signature(private_key, &message) {
            Ok(sig) => sig,
            Err(e) => {
                log::error!("Error generating signature: {}", e);
                return Ok(());
            }
        };

        if let Err(e) = self
            .submit_transaction_with_retry(
                block,
                private_key,
                keypair,
                signature,
                hex_decoded_state_root,
            )
            .await
        {
            log::info!("Error submitting tx: {:?}", e);
        }

        Ok(())
    }

    async fn submit_transaction_with_retry(
        &self,
        block: BlockT<PolkadotConfig, OnlineClient<SxtConfig>>,
        private_key: &SigningKey,
        keypair: &Keypair,
        signature: EthereumSignature,
        state_root: Vec<u8>,
    ) -> Result<(), AttestationError> {
        let header = block.header();
        let mut attempt = 0;

        loop {
            let attestation =
                create_attestation(header, private_key, signature, state_root.clone())?;

            let tx_params = Params::new().nonce(self.nonce.get()).build();
            let tx = runtime::api::tx()
                .attestations()
                .attest_block(block.number(), attestation);

            match self
                .api
                .tx()
                .sign_and_submit_then_watch(&tx, keypair, tx_params)
                .await
            {
                Ok(_) => {
                    self.nonce.inc_by_with_ordering(1, Ordering::SeqCst);
                    info!(
                        "Transaction for block {:?} succeeded on attempt {}",
                        block.number(),
                        attempt + 1
                    );
                    return Ok(());
                }
                Err(err) if attempt < 1 => {
                    attempt += 1;
                    self.nonce.dec_by(1);
                    warn!("Retry attempt {} due to error: {}", attempt, err);
                }
                Err(err) => return Err(AttestationError::TransactionFailed(err.to_string())),
            }
        }
    }

    /// Get the websocket url
    pub fn websocket(&self) -> String {
        self.websocket.clone()
    }
}

fn create_attestation(
    header: &SubstrateHeader<u32, BlakeTwo256>,
    private_key: &SigningKey,
    signature: EthereumSignature,
    state_root: Vec<u8>,
) -> Result<runtime::api::runtime_types::sxt_core::attestation::Attestation<H256>, AttestationError>
{
    let sxt_core::attestation::EthereumSignature { r, s, v } = signature;
    let proposed_pub_key = get_proposed_pub_key(private_key)?;

    let block_number = header.number;
    let block_hash = header.hash();

    let address20 = sxt_core::attestation::uncompressed_public_key_to_address(&proposed_pub_key)?;
    let address20 = BoundedVec(address20.as_slice().to_vec());

    Ok(
        runtime::api::runtime_types::sxt_core::attestation::Attestation::EthereumAttestation {
            signature: runtime::api::runtime_types::sxt_core::attestation::EthereumSignature {
                r,
                s,
                v,
            },
            proposed_pub_key,
            state_root: BoundedVec(state_root),
            address20,
            block_number,
            block_hash,
        },
    )
}

// Placeholder function for the register command
async fn register(eth_key_path: &str, substrate_key_path: &str) -> Result<(), AttestationError> {
    let eth_signing_key = load_ethereum_key(eth_key_path)?;
    let substrate_key = load_substrate_key(substrate_key_path)?;
    let account_id = substrate_key.public_key().to_account_id().0;

    let private_key = eth_signing_key.to_bytes();
    let public_key: &[u8] = &eth_signing_key.verifying_key().to_sec1_bytes();

    let registration = sxt_core::attestation::create_ethereum_attestation_registration(
        &account_id,
        &private_key,
        public_key,
    )?;

    let RegisterExternalAddress::EthereumAddress {
        signature,
        proposed_pub_key,
        address20,
    } = registration;
    let sxt_core::attestation::EthereumSignature { r, s, v } = signature;

    // Format all of these values as hex
    info!(
        "Send these registration details to an SxT network admin\naccount_id={}\nr=0x{}\ns=0x{}\nv=0x{:x}\npub_key=0x{}\neth_address=0x{}",
        substrate_key.public_key().to_account_id(),
        hex::encode(r),
        hex::encode(s),
        v,
        hex::encode(proposed_pub_key), // convert the public key to hex using hex::encode for slices
        hex::encode(address20),
    );

    Ok(())
}

fn create_message(state_root: impl AsRef<[u8]>, block_number: u32) -> Vec<u8> {
    let mut msg = Vec::with_capacity(state_root.as_ref().len() + std::mem::size_of::<u32>());
    msg.extend_from_slice(state_root.as_ref());
    msg.extend_from_slice(&block_number.to_le_bytes());
    msg
}

fn generate_signature(
    private_key: &SigningKey,
    message: &[u8],
) -> Result<sxt_core::attestation::EthereumSignature, AttestationError> {
    sign_eth_message(&private_key.to_bytes(), message)
        .map_err(|_| AttestationError::AttestationCreationError)
}

fn get_proposed_pub_key(private_key: &SigningKey) -> Result<[u8; 33], AttestationError> {
    let pub_key: &[u8] = &private_key.verifying_key().to_sec1_bytes();

    pub_key
        .try_into()
        .map_err(|_| AttestationError::SigningKeyParseError)
}
fn load_ethereum_key(path: &str) -> Result<SigningKey, AttestationError> {
    let mut file = File::open(path)?;
    let mut hex_string = String::new();
    file.read_to_string(&mut hex_string)?;

    let key_bytes =
        Vec::from_hex(hex_string.trim()).map_err(|_| AttestationError::EthereumHexDecodeError)?;

    let key_array = GenericArray::from_slice(&key_bytes);
    SigningKey::from_bytes(key_array).map_err(|_| AttestationError::SigningKeyParseError)
}

fn load_substrate_key(file_path: &str) -> Result<Keypair, AttestationError> {
    let mut file = File::open(file_path)?;
    let mut hex_string = String::new();
    file.read_to_string(&mut hex_string)?;

    let key_bytes =
        Vec::from_hex(hex_string.trim()).map_err(|_| AttestationError::HexDecodeError)?;

    if key_bytes.len() != 32 {
        return Err(AttestationError::InvalidKeyLength);
    }

    let key_bytes: [u8; 32] = key_bytes.try_into().unwrap();
    Keypair::from_secret_key(key_bytes).map_err(|_| AttestationError::KeypairCreationError)
}

async fn verify(block_number: u32, websocket: &str) -> Result<(), AttestationError> {
    // Initialize terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut progress = vec!["Connecting to the chain...".to_string()];

    // Connect to API
    let api = OnlineClient::<SxtConfig>::from_url(websocket).await?;
    progress.push(format!("Connected to chain at {}", websocket));
    update_ui(&mut terminal, &progress)?;

    // Fetch attestations
    progress.push(format!(
        "Fetching attestations for block {}...",
        block_number
    ));
    update_ui(&mut terminal, &progress)?;

    let attestations_addr = runtime::api::storage()
        .attestations()
        .attestations(block_number);

    let attestations = api
        .storage()
        .at_latest()
        .await?
        .fetch(&attestations_addr)
        .await?;

    if attestations.is_none() {
        progress.push(format!("No attestations found for block {}", block_number));
        update_ui(&mut terminal, &progress)?;
    } else {
        let attestations = attestations.unwrap();

        if attestations.0.is_empty() {
            progress.push(format!("No attestations found for block {}", block_number));
            update_ui(&mut terminal, &progress)?;
            return Ok(());
        } else {
            progress.push(format!("Found {} attestations", attestations.0.len()));
            update_ui(&mut terminal, &progress)?;

            // Process attestations
            progress.push("Verifying attestations...".to_string());
            update_ui(&mut terminal, &progress)?;

            if let Err(err) =
                verify_attestations(block_number, &attestations.0, &mut progress, &mut terminal)
            {
                progress.push(format!("Error: {:?}", err));
                update_ui(&mut terminal, &progress)?;
            } else {
                progress.push(format!(
                    "Successfully verified all attestations for block {}",
                    block_number
                ));
            }
        }
    }

    progress.push("Press q to quit".into());
    update_ui(&mut terminal, &progress)?;

    // Wait for user input
    loop {
        if let Event::Key(key) = read()? {
            if key.code == KeyCode::Char('q') {
                break;
            }
        }
    }

    cleanup_terminal(&mut terminal)?;
    Ok(())
}

/// Verifies a list of attestations.
fn verify_attestations<B: ratatui::backend::Backend>(
    block_number: u32,
    attestations: &[Attestation<H256>],
    progress: &mut Vec<String>,
    terminal: &mut Terminal<B>,
) -> Result<(), AttestationError> {
    let mut first_state_root: Option<&BoundedVec<u8>> = None;

    for (i, attestation) in attestations.iter().enumerate() {
        let runtime::api::runtime_types::sxt_core::attestation::Attestation::EthereumAttestation {
            signature,
            proposed_pub_key,
            state_root,
            address20,
            block_number,
            block_hash,
        } = attestation;

        // Create message and verify signature
        let msg = create_message(state_root.0.clone(), *block_number);
        progress.push(format!(
            "Verifying attestation {}/{}...",
            i + 1,
            attestations.len()
        ));
        update_ui(terminal, progress)?;

        verify_signature(&msg, signature, proposed_pub_key, *block_number)?;

        // Check state root consistency
        if let Some(first_root) = first_state_root {
            if first_root.0 != state_root.0 {
                return Err(AttestationError::StateRootMismatch);
            }
        } else {
            first_state_root = Some(state_root);
        }

        progress.push(format!("Attestation {}: ✅ Verified successfully.", i + 1));
        update_ui(terminal, progress)?;
    }

    Ok(())
}

/// Updates the terminal UI with the current progress.
fn update_ui<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    progress: &[String],
) -> io::Result<()> {
    terminal.draw(|f| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Percentage(90), Constraint::Percentage(10)].as_ref())
            .split(f.size());

        let items: Vec<ListItem> = progress
            .iter()
            .map(|line| ListItem::new(line.clone()))
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .title("Verification Progress")
                    .borders(Borders::ALL),
            )
            .style(Style::default().fg(Color::White));

        f.render_widget(list, chunks[0]);
    });

    Ok(())
}

/// Cleans up the terminal UI on exit.
fn cleanup_terminal<B>(terminal: &mut Terminal<B>) -> io::Result<()>
where
    B: ratatui::backend::Backend + std::io::Write,
{
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;
    Ok(())
}
/// Verifies the signature of an attestation.
fn verify_signature(
    msg: &[u8],
    signature: &runtime::api::runtime_types::sxt_core::attestation::EthereumSignature,
    proposed_pub_key: &[u8; 33],
    block_number: u32,
) -> Result<(), AttestationError> {
    let runtime::api::runtime_types::sxt_core::attestation::EthereumSignature { r, s, v } =
        signature;
    let signature = sxt_core::attestation::EthereumSignature {
        r: *r,
        s: *s,
        v: *v,
    };

    verify_eth_signature(msg, &signature, proposed_pub_key)?;

    Ok(())
}
