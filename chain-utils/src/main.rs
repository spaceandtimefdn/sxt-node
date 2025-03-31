//! This utility is built to read a DDL file from a given path and submit it to the SxT Chain
//! using a given private key.

mod common;
mod fetch_submissions;
mod load_tables;
mod print_batch;
mod test_staking;

use std::io::Write;
use std::path::PathBuf;
use std::process;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Error};
use arrow::ipc::reader::StreamReader;
use arrow::util::pretty::print_batches;
use clap::{Parser, Subcommand};
use log::{error, info};
use subxt::backend::rpc::reconnecting_rpc_client::RpcClient;
use subxt::utils::{AccountId32, H256};
use subxt::{OnlineClient, PolkadotConfig};
use sxt_core::sxt_chain_runtime::api::runtime_types::sxt_core::tables::{
    IndexerMode,
    InsertQuorumSize,
    Source,
    SourceAndMode,
    TableIdentifier,
};
use sxt_core::sxt_chain_runtime::api::{self, tx};

/// CLI entrypoint
#[derive(clap::Parser)]
#[command(
    name = "sxt-cli",
    version,
    about = "CLI for interacting with SxT chain"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

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

    /// Stub for future utility to print batch
    PrintBatch {
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
