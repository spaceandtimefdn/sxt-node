use polkadot_sdk::sc_cli::RunCmd;
use polkadot_sdk::{frame_benchmarking_cli, sc_cli, sc_storage_monitor};
use proof_of_sql_static_setups::io::ProofOfSqlPublicSetupArgs;
use snafu::{OptionExt, Snafu};

#[derive(Debug, clap::Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub subcommand: Option<Subcommand>,

    #[clap(flatten)]
    pub run: RunCmd,

    #[clap(long)]
    pub event_forwarder: bool,

    #[clap(long)]
    pub event_forwarder_key: Option<String>,

    #[clap(long)]
    pub event_forwarder_rpc: Option<String>,

    #[allow(missing_docs)]
    #[clap(flatten)]
    pub storage_monitor: sc_storage_monitor::StorageMonitorParams,

    /// Configuration for loading proof-of-sql public setups.
    #[clap(flatten)]
    pub proof_of_sql_public_setup_args: ProofOfSqlPublicSetupArgs,

    /// Node-local `KEY=VALUE` configuration entries, exposed to offchain
    /// workers via the `native::config` runtime interface. Repeat the flag once per entry.
    #[clap(long, value_parser = parse_key_val)]
    pub ocw_config: Vec<(String, String)>,
}

/// Error parsing a `KEY=VALUE` entry.
#[derive(Debug, Snafu, PartialEq)]
#[snafu(display("expected 'KEY=VALUE', got '{s}'"))]
pub struct ParseKeyValError {
    s: String,
}

fn parse_key_val(s: &str) -> Result<(String, String), ParseKeyValError> {
    s.split_once('=')
        .context(ParseKeyValSnafu { s })
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
}

#[derive(Debug, clap::Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum Subcommand {
    /// Key management cli utilities
    #[command(subcommand)]
    Key(sc_cli::KeySubcommand),

    /// Build a chain specification.
    BuildSpec(sc_cli::BuildSpecCmd),

    /// Validate blocks.
    CheckBlock(sc_cli::CheckBlockCmd),

    /// Export blocks.
    ExportBlocks(sc_cli::ExportBlocksCmd),

    /// Export the state of a given block into a chain spec.
    ExportState(sc_cli::ExportStateCmd),

    /// Import blocks.
    ImportBlocks(sc_cli::ImportBlocksCmd),

    /// Remove the whole chain.
    PurgeChain(sc_cli::PurgeChainCmd),

    /// Revert the chain to a previous state.
    Revert(sc_cli::RevertCmd),

    /// Sub-commands concerned with benchmarking.
    #[command(subcommand)]
    Benchmark(frame_benchmarking_cli::BenchmarkCmd),

    /// Db meta columns information.
    ChainInfo(sc_cli::ChainInfoCmd),
}

#[cfg(test)]
mod tests {
    use super::{parse_key_val, ParseKeyValError};

    #[test]
    fn we_can_parse_key_val() {
        assert_eq!(
            parse_key_val("cfg"),
            Err(ParseKeyValError {
                s: "cfg".to_owned()
            })
        );
        assert_eq!(parse_key_val("cfg="), Ok(("cfg".to_owned(), "".to_owned())));
        assert_eq!(
            parse_key_val("cfg=foo"),
            Ok(("cfg".to_owned(), "foo".to_owned()))
        );
        assert_eq!(
            parse_key_val("cfg=foo=bar"),
            Ok(("cfg".to_owned(), "foo=bar".to_owned()))
        );
    }
}
