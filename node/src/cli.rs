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

    /// Prover-db consumer settings. Setting `--prover-db-url` is what
    /// turns this node into a prover-db indexer; the other fields in
    /// the group only take effect when the URL is set. See the field
    /// docs on [`ProverDbConsumerCli`] for grammar.
    #[clap(flatten)]
    pub prover_db: ProverDbConsumerCli,

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

/// Flattened consumer-side configuration. Lives on the top-level
/// `Cli` via `#[clap(flatten)]`. Validated at service init into
/// `sxt_core::prover_db_indexer::ProverDbConsumerConfig` before being
/// written to offchain local storage; see
/// `service::configure_prover_db_consumer`.
///
/// On the CLI side the fields are individually optional / repeatable —
/// clap can't express "if URL is set, the group is active" in the type.
/// The resolver in `service.rs` enforces that constraint and produces a
/// concrete `ProverDbConsumerConfig` (URL non-optional) or refuses to
/// start.
#[derive(Debug, clap::Args)]
pub struct ProverDbConsumerCli {
    /// URL of the prover-db indexer to forward indexed events to.
    /// Also configurable via the `PROVER_DB_URL` env var.
    ///
    /// Presence of this flag is what enables the consumer. If omitted,
    /// the OCW stays dormant regardless of `--prover-db-include`.
    #[clap(long, env)]
    pub prover_db_url: Option<url::Url>,

    /// Wildcard patterns naming which tables this node should forward
    /// to the indexer. Repeat the flag, or pass a single comma-
    /// separated value, or set `PROVER_DB_INCLUDE`.
    ///
    /// Each entry parses as a [`TableIdentifierFilter`]: two
    /// dot-separated sides, each either a literal identifier or `*`.
    /// Both sides match independently, so:
    ///   - `*.*`             — every captured event (default if omitted).
    ///   - `NAMESPACE.*`     — every table in NAMESPACE.
    ///   - `*.NAME`          — every table called NAME, regardless of namespace.
    ///   - `NAMESPACE.NAME`  — exactly that one table.
    ///
    /// Identifiers are uppercased before being written to offchain
    /// storage so the byte-match against the on-chain canonical form
    /// works without per-operator vigilance.
    ///
    /// [`TableIdentifierFilter`]: sxt_core::prover_db_indexer::TableIdentifierFilter
    #[clap(long, env, value_delimiter = ',', default_value = "*.*")]
    pub prover_db_include: Vec<sxt_core::prover_db_indexer::TableIdentifierFilter>,
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
