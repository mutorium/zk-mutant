use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::nargo::run_nargo_test;
use crate::scan::{ProjectOverview, scan_project};

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

            match scan_project(&project) {
                Ok(report) => print_scan_summary(&report),
                Err(e) => {
                    eprintln!("failed to analyze Noir project at {:?}: {e}", project);
                }
            }
        }

        Command::Run { project } => {
            println!("zk-mutant: run");
            println!("project: {:?}", project);

            match run_nargo_test(&project) {
                Ok(result) => {
                    println!(
                        "nargo test finished in {:?} (exit code: {:?}, success: {})",
                        result.duration, result.exit_code, result.success
                    );

                    if !result.success {
                        eprintln!("nargo test failed");

                        if !result.stdout.is_empty() {
                            eprintln!("stdout from nargo:\n{}", result.stdout);
                        }

                        if !result.stderr.is_empty() {
                            eprintln!("stderr from nargo:\n{}", result.stderr);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("failed to run `nargo test` in {:?}: {e}", project);
                }
            }
        }
    }

    Ok(())
}

/// Print a short summary based on the project overview.
fn print_scan_summary(overview: &ProjectOverview) {
    println!("--- project overview ---");
    println!("project root:            {}", overview.root.display());
    println!("nr files (.nr):          {}", overview.nr_files);
    println!("test files:              {}", overview.test_files);
    println!("code lines:              {}", overview.code_lines);
    println!("test functions:          {}", overview.test_functions);
    println!("test code lines:         {}", overview.test_lines);
    println!("non-test code lines:     {}", overview.non_test_lines);
    println!(
        "test code ratio:         {:.2}% (test_lines / code_lines)",
        overview.test_code_ratio
    );
}
