use polkadot_sdk::sc_cli::RunCmd;
use polkadot_sdk::{frame_benchmarking_cli, sc_cli, sc_storage_monitor};
use proof_of_sql_static_setups::io::ProofOfSqlPublicSetupArgs;

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
    /// Grammar (always exactly one dot, two segments):
    ///   - `*.*`             — every captured event (default if omitted).
    ///   - `NAMESPACE.*`     — every table in NAMESPACE.
    ///   - `NAMESPACE.NAME`  — exactly that one table.
    /// `*.NAME` is rejected: the runtime rule type can't express
    /// "any namespace, this name".
    ///
    /// Identifiers are uppercased before being written to offchain
    /// storage so the byte-match against the on-chain canonical form
    /// works without per-operator vigilance.
    #[clap(long, env, value_delimiter = ',')]
    pub prover_db_include: Vec<String>,
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
