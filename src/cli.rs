use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::discover::discover_mutants;
use crate::nargo::run_nargo_test;
use crate::options::Options;
use crate::project::Project;
use crate::report::print_surviving_mutants;
use crate::runner::run_all_mutants_in_temp;
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

/// Parse CLI arguments and dispatch the selected command.
pub fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Scan { project } => {
            let options = Options::new(project);

            println!("zk-mutant: scan");
            println!("project: {:?}", options.project_root);

            match scan_project(&options.project_root) {
                Ok(report) => print_scan_summary(&report),
                Err(e) => {
                    eprintln!(
                        "failed to analyze Noir project at {:?}: {e}",
                        options.project_root
                    );
                }
            }

            Ok(())
        }

        Command::Run { project } => {
            let options = Options::new(project);

            println!("zk-mutant: run");
            println!("project: {:?}", options.project_root);

            // Load Noir project and metrics via noir-metrics.
            let project = match Project::from_root(options.project_root.clone()) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!(
                        "failed to load Noir project at {:?}: {e}",
                        options.project_root
                    );
                    return Err(e);
                }
            };

            // Baseline `nargo test` run before mutation testing.
            match run_nargo_test(project.root()) {
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

                        // If baseline tests fail, don't attempt mutation testing.
                        return Err(anyhow::anyhow!("baseline `nargo test` failed"));
                    }
                }
                Err(e) => {
                    eprintln!("failed to run `nargo test` in {:?}: {e}", project.root());
                    return Err(e);
                }
            }

            // Discover mutation opportunities.
            let mut mutants = discover_mutants(&project);
            println!("discovered {} mutants", mutants.len());

            if mutants.is_empty() {
                println!("no mutants discovered, exiting");
                return Ok(());
            }

            // Run all mutants sequentially (naive implementation).
            let summary = run_all_mutants_in_temp(&project, &mut mutants)?;

            println!("--- mutation run summary ---");
            println!("mutants total:    {}", mutants.len());
            println!("mutants killed:   {}", summary.killed);
            println!("mutants survived: {}", summary.survived);
            println!("mutants invalid:  {}", summary.invalid);

            // Extra observability: list surviving mutants with their textual change.
            print_surviving_mutants(&mutants);

            Ok(())
        }
    }
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
