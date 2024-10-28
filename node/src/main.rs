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

/// RPC setup
mod rpc;

/// Service instantiation
mod service;

fn main() -> sc_cli::Result<()> {
    command::run()
}
