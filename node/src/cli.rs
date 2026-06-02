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

    /// URL of the prover-db indexer to forward indexed events to. Also
    /// configurable via the `PROVER_DB_URL` environment variable.
    #[clap(long, env)]
    pub prover_db_url: Option<url::Url>,

    /// Path to a JSON file listing which tables/namespaces this node
    /// should forward to its prover-db indexer. Schema:
    ///
    /// ```json
    /// [
    ///   { "kind": "namespace", "value": "ETHEREUM_MT" },
    ///   { "kind": "table",     "namespace": "OTHER_NS", "name": "SPECIAL" }
    /// ]
    /// ```
    ///
    /// An empty array (or omitting this flag) means "forward every
    /// captured event" — the on-chain capture itself is unfiltered, so
    /// the choice is purely per-node. Identifiers are uppercased before
    /// being written to offchain storage, to match how the chain stores
    /// them.
    #[clap(long, env)]
    pub prover_db_include_file: Option<std::path::PathBuf>,

    #[allow(missing_docs)]
    #[clap(flatten)]
    pub storage_monitor: sc_storage_monitor::StorageMonitorParams,

    /// Configuration for loading proof-of-sql public setups.
    #[clap(flatten)]
    pub proof_of_sql_public_setup_args: ProofOfSqlPublicSetupArgs,
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
