use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

/// Top-level CLI arguments for the `zk-mutant` binary.
#[derive(Debug, Parser)]
#[command(
    name = "zk-mutant",
    version,
    about = "Mutation testing for Noir circuits"
)]
pub struct Cli {
    /// Subcommand to execute.
    #[command(subcommand)]
    pub command: Command,
}

/// Subcommands supported by `zk-mutant`.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run a scan of the project.
    Scan {
        /// Path to the Noir project root or any path inside it.
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// Run mutation testing.
    Run {
        /// Path to the Noir project root or any path inside it.
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
}

/// Parse CLI arguments and print the selected command.
pub fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Scan { project } => {
            println!("zk-mutant: scan");
            println!("project: {:?}", project);
        }

        Command::Run { project } => {
            println!("zk-mutant: run");
            println!("project: {:?}", project);
        }
    }

    Ok(())
}
