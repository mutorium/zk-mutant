use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::discover::discover_mutants;
use crate::nargo::run_nargo_test;
use crate::options::Options;
use crate::project::Project;
use crate::report::{print_all_mutants, print_surviving_mutants};
use crate::run_report::{BaselineReport, MutationRunReport, RunSummary};
use crate::runner::{Progress, run_all_mutants_in_temp};
use crate::scan::{ProjectOverview, scan_project};

const EXIT_OK: i32 = 0;
const EXIT_ERROR: i32 = 1;
const EXIT_SURVIVORS: i32 = 2;

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

        /// Print a detailed list of all mutants and their outcomes.
        #[arg(long, short = 'v')]
        verbose: bool,

        /// Run only the first N discovered mutants (deterministic order).
        #[arg(long)]
        limit: Option<usize>,

        /// Emit a machine-readable JSON report to stdout.
        #[arg(long)]
        json: bool,

        /// Exit with code 2 if any mutants survive (useful for CI).
        #[arg(long)]
        fail_on_survivors: bool,
    },
}

fn print_json_and_exit(report: MutationRunReport, exit_code: i32) -> ! {
    let json = serde_json::to_string_pretty(&report).expect("serialize report to json");
    println!("{json}");
    std::process::exit(exit_code);
}

/// Print human-oriented output.
/// - normal mode: stdout
/// - `--json` mode: stderr (so stdout stays machine-readable)
fn human_ln(json: bool, msg: impl std::fmt::Display) {
    if json {
        eprintln!("{msg}");
    } else {
        println!("{msg}");
    }
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

        Command::Run {
            project,
            verbose,
            limit,
            json,
            fail_on_survivors,
        } => {
            let options = Options::new(project);
            let project_root = options.project_root.clone();

            human_ln(json, "zk-mutant: run");
            human_ln(json, format!("project: {:?}", project_root));

            // Load Noir project and metrics via noir-metrics.
            let project = match Project::from_root(project_root.clone()) {
                Ok(p) => p,
                Err(e) => {
                    if json {
                        let report = MutationRunReport::failure(
                            project_root,
                            BaselineReport {
                                success: false,
                                exit_code: None,
                                duration_ms: 0,
                            },
                            format!("failed to load Noir project: {e}"),
                        );
                        print_json_and_exit(report, EXIT_ERROR);
                    }

                    eprintln!("failed to load Noir project at {:?}: {e}", project_root);
                    return Err(e);
                }
            };

            // Baseline `nargo test` run before mutation testing.
            let baseline_result = match run_nargo_test(project.root()) {
                Ok(r) => r,
                Err(e) => {
                    if json {
                        let report = MutationRunReport::failure(
                            project_root,
                            BaselineReport {
                                success: false,
                                exit_code: None,
                                duration_ms: 0,
                            },
                            format!("failed to run `nargo test`: {e}"),
                        );
                        print_json_and_exit(report, EXIT_ERROR);
                    }

                    eprintln!("failed to run `nargo test` in {:?}: {e}", project.root());
                    return Err(e);
                }
            };

            let baseline = BaselineReport::from_nargo(&baseline_result);

            human_ln(
                json,
                format!(
                    "nargo test finished in {:?} (exit code: {:?}, success: {})",
                    baseline_result.duration, baseline_result.exit_code, baseline_result.success
                ),
            );

            if !baseline_result.success {
                if json {
                    let report = MutationRunReport::failure(
                        project_root,
                        baseline,
                        "baseline `nargo test` failed".to_string(),
                    );
                    print_json_and_exit(report, EXIT_ERROR);
                }

                eprintln!("nargo test failed");

                if !baseline_result.stdout.is_empty() {
                    eprintln!("stdout from nargo:\n{}", baseline_result.stdout);
                }
                if !baseline_result.stderr.is_empty() {
                    eprintln!("stderr from nargo:\n{}", baseline_result.stderr);
                }

                return Err(anyhow::anyhow!("baseline `nargo test` failed"));
            }

            // Discover mutation opportunities.
            let mut mutants = discover_mutants(&project);
            let discovered = mutants.len();
            human_ln(json, format!("discovered {} mutants", discovered));

            if discovered == 0 {
                if json {
                    let report = MutationRunReport::success(
                        project_root,
                        0,
                        0,
                        baseline,
                        RunSummary::default(),
                        Vec::new(),
                    );
                    print_json_and_exit(report, EXIT_OK);
                }

                human_ln(json, "no mutants discovered, exiting");
                return Ok(());
            }

            if let Some(limit) = limit {
                if limit == 0 {
                    if json {
                        let report = MutationRunReport::success(
                            project_root,
                            discovered,
                            0,
                            baseline,
                            RunSummary::default(),
                            Vec::new(),
                        );
                        print_json_and_exit(report, EXIT_OK);
                    }

                    human_ln(json, "mutant limit is 0, exiting");
                    return Ok(());
                }

                if mutants.len() > limit {
                    mutants.truncate(limit);
                }

                human_ln(
                    json,
                    format!("running {} mutants (of {})", mutants.len(), discovered),
                );
            }

            // Ensure stdout is clean JSON in `--json` mode: progress goes to stderr.
            let progress = if json {
                Progress::Stderr
            } else {
                Progress::Stdout
            };

            // Run all mutants sequentially (naive implementation).
            let executed = mutants.len();
            let summary = run_all_mutants_in_temp(&project, &mut mutants, progress)?;

            // CI policy
            let wants_ci_fail = fail_on_survivors && summary.survived > 0;
            let exit_code = if wants_ci_fail {
                EXIT_SURVIVORS
            } else {
                EXIT_OK
            };

            if json {
                let report = MutationRunReport::success(
                    project_root,
                    discovered,
                    executed,
                    baseline,
                    summary,
                    mutants,
                );
                print_json_and_exit(report, exit_code);
            }

            println!("--- mutation run summary ---");
            println!("mutants total:    {}", executed);
            println!("mutants killed:   {}", summary.killed);
            println!("mutants survived: {}", summary.survived);
            println!("mutants invalid:  {}", summary.invalid);

            if verbose {
                print_all_mutants(&project, &mutants);
            }

            print_surviving_mutants(&project, &mutants);

            if wants_ci_fail {
                eprintln!(
                    "mutation testing failed policy: {} mutant(s) survived (--fail-on-survivors)",
                    summary.survived
                );
                std::process::exit(EXIT_SURVIVORS);
            }

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
