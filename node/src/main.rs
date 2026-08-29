//! Substrate Node Template CLI library.
#![warn(missing_docs)]
#![warn(unused_crate_dependencies)]

// functionally unused, but these features need to be enabled for rocksdb support
use {sc_cli as _, sc_client_db as _};

/// benchmarking
mod benchmarking;

/// chain spec
mod chain_spec;

/// CLI flags
mod cli;

/// Service Configuration
mod command;

mod client_provider;

/// Service instantiation
mod service;

#[expect(
    clippy::result_large_err,
    reason = "sc_cli::Result is from substrate and cannot be modified"
)]
fn main() -> polkadot_sdk::sc_cli::Result<()> {
    command::run()
}
