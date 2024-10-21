//! TODO revisit this

use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;

use clap::Parser;
use commit_grouper::CommitmentParser;

/// A simple CLI that takes a string and an output file location
#[derive(Parser, Debug)]
#[command(name = "commit-grouper")]
#[command(version = "1.0")]
#[command(about = "Takes a glob to a list of commit files and groups them into a single object", long_about = None)]
struct Cli {
    /// The input string to be written to the file
    #[arg(short, long)]
    pattern: String,

    /// The output file path where the string will be written
    #[arg(short, long)]
    output: PathBuf,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    let mut output = File::create(cli.output)?;

    match CommitmentParser::parse_commits_from_glob(&cli.pattern) {
        Ok(c) => {
            let serialized = serde_json::to_string(&c).expect("Could not serialize to json");
            writeln!(output, "{}", serialized).expect("Could not write to output file");
        }
        Err(e) => {
            eprintln!("Error parsing commits: {}", e)
        }
    };

    Ok(())
}
