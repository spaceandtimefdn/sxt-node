#![doc = include_str!("README.md")]
use clap::{Parser, ValueEnum};

mod write_cases;
use on_chain_table::StringToScalarConversion;
pub use write_cases::write_cases;

mod process_insert;

/// The function to generate cases for.
#[derive(Debug, Clone, ValueEnum)]
enum Function {
    /// native::interface::process_insert
    ProcessInsert,
    /// native::interface::process_insert
    ProcessInsertVersion2,
}

/// Generate "cases" for native interfaces.
#[derive(Debug, Clone, Parser)]
#[command(about, long_about = include_str!("README.md"))]
struct Args {
    /// The function to generate cases for.
    function: Function,
    /// Does not need to be supplied, already exists in cargo environment.
    #[arg(env)]
    cargo_workspace_dir: std::path::PathBuf,
}

fn main() {
    let Args {
        function,
        cargo_workspace_dir,
    } = Args::parse();

    let cases_dir = cargo_workspace_dir.join("native/backwards_compatibility_cases");

    match function {
        Function::ProcessInsert => {
            process_insert::write_process_insert_cases(
                cases_dir,
                native::interface::process_insert,
                StringToScalarConversion::Posql99,
            );
        }
        Function::ProcessInsertVersion2 => {
            process_insert::write_process_insert_cases(
                cases_dir,
                native::interface::process_insert_version_2,
                StringToScalarConversion::Core,
            );
        }
    }
}
