//! This utility is built to read a DDL file from a given path and submit it to the SxT Chain
//! using a given private key.

mod common;
mod contracts;
mod drop_tables;
mod fetch_submissions;
mod load_contract;
mod load_tables;
mod print_batch;
mod test_staking;
mod update_uuids;

use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};
use log::error;
use subxt::utils::H256;

use crate::contracts::{SxtNetwork, SystemContract};

/// CLI entrypoint
#[derive(clap::Parser)]
#[command(
    name = "sxt-cli",
    version,
    about = "CLI for interacting with SxT chain"
)]
struct Cli {
    /// The chain utilities as subcommands.
    #[command(subcommand)]
    command: Commands,
}

/// Wrappers for the available commands and their corresponding arguments
#[derive(Subcommand)]
enum Commands {
    /// Load table definitions from a DDL file and submit to the SxT chain
    LoadTables {
        /// Path to the SQL DDL file
        #[arg(short, long)]
        file: PathBuf,

        /// Private key URI to sign transactions
        #[arg(short, long)]
        private_key: String,

        /// Node RPC endpoint
        #[arg(short, long, default_value = "ws://127.0.0.1:9944")]
        rpc: url::Url,
    },
    /// Drop tables listed in the supplied DDL File and submit to the SXT Chain
    DropTables {
        /// Path to the SQL DDL file
        #[arg(short, long)]
        file: PathBuf,

        /// Private key URI to sign transactions
        #[arg(short, long)]
        private_key: String,

        /// Node RPC endpoint
        #[arg(short, long, default_value = "ws://127.0.0.1:9944")]
        rpc: url::Url,
    },
    // Load tables and ABIs for Smart Contracts like Staking, Messaging, and zkPay
    LoadContract {
        /// Path to the SQL DDL file for the contract's DDL
        #[arg(short, long)]
        file: PathBuf,

        /// Private key URI to sign transactions
        #[arg(short, long)]
        private_key: String,

        /// Node RPC endpoint
        #[arg(short, long, default_value = "ws://127.0.0.1:9944")]
        rpc: url::Url,

        /// The network to load contracts for
        #[arg(short, long)]
        network: SxtNetwork,

        /// The contract to upload to the network
        #[arg(short, long)]
        contract: SystemContract,
    },

    /// Read UUIDs from a DDL file and update the corresponding tables to the supplied UUIDs
    UpdateUuids {
        /// Path to the SQL DDL file
        #[arg(short, long)]
        file: PathBuf,

        /// Private key URI to sign transactions
        #[arg(short, long)]
        private_key: String,

        /// Node RPC endpoint
        #[arg(short, long, default_value = "ws://127.0.0.1:9944")]
        rpc: url::Url,

        /// The table versions to update, defaults to 0
        #[arg(short, long, default_value = "0")]
        version: u16,
    },

    /// Stub for future utility to print batch
    PrintBatch {
        /// The arrow record batch IPC bytes.
        #[arg(short, long)]
        row_data: String,
    },

    /// Fetch SubmitData events from a given block
    FetchSubmissions {
        /// Block hash (0x-prefixed)
        #[arg(short, long)]
        block: H256,

        /// Node RPC endpoint
        #[arg(short, long, default_value = "ws://127.0.0.1:9944")]
        rpc: url::Url,
    },
    /// Submit staking + session keys message for a test validator (Sepolia impersonation)
    TestStaking {
        /// Private key URI to sign transactions
        #[arg(short, long)]
        private_key: String,

        /// Node RPC endpoint
        #[arg(short, long, default_value = "ws://127.0.0.1:9944")]
        rpc: url::Url,

        /// Session keys from rotateKeys
        #[arg(short = 's', long)]
        session_keys: String,

        /// Ethereum wallet address to impersonate
        #[arg(short = 'e', long)]
        eth_wallet: String,
    },
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::LoadContract {
            file,
            private_key,
            rpc,
            network,
            contract,
        } => {
            if let Err(e) =
                load_contract::load_contract(file, &private_key, &rpc, network, contract).await
            {
                error!("Failed to load contract: {}", e);
                process::exit(1);
            }
        }
        Commands::LoadTables {
            file,
            private_key,
            rpc,
        } => {
            if let Err(e) = load_tables::load_tables(file, &private_key, &rpc).await {
                error!("Failed to load tables: {}", e);
                process::exit(1);
            }
        }
        Commands::DropTables {
            file,
            private_key,
            rpc,
        } => {
            if let Err(e) = drop_tables::drop_tables(file, &private_key, &rpc).await {
                error!("Failed to drop tables: {}", e);
                process::exit(1);
            }
        }
        Commands::UpdateUuids {
            file,
            private_key,
            rpc,
            version,
        } => {
            if let Err(e) = update_uuids::update_uuids(file, &private_key, &rpc, version).await {
                error!("Failed to load tables: {}", e);
                process::exit(1);
            }
        }
        Commands::PrintBatch { row_data } => {
            if let Err(e) = print_batch::print_batch(row_data.as_str()) {
                error!("Failed to print batch: {}", e);
            }
        }
        Commands::FetchSubmissions { block, rpc } => {
            if let Err(e) = fetch_submissions::fetch_submissions(block, &rpc).await {
                error!("Failed to fetch submissions: {}", e);
            }
        }
        Commands::TestStaking {
            private_key,
            rpc,
            session_keys,
            eth_wallet,
        } => {
            if let Err(e) =
                test_staking::test_staking(&private_key, &rpc, &session_keys, &eth_wallet).await
            {
                error!("Test staking failed: {}", e);
            }
        }
    }
}
