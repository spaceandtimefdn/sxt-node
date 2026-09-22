//! Substrate Node Template CLI library.
#![warn(missing_docs)]

/// benchmarking
mod benchmarking;

/// chain spec
mod chain_spec;

/// CLI flags
mod cli;

/// Service Configuration
mod command;

/// Service instantiation
mod service;

#[expect(
    clippy::result_large_err,
    reason = "sc_cli::Result is from substrate and cannot be modified"
)]
fn main() -> polkadot_sdk::sc_cli::Result<()> {
    command::run()
}
