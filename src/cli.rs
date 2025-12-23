use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::Serialize;

use crate::discover::discover_mutants;
use crate::mutant::Mutant;
use crate::nargo::{compiler_version_from_nargo_toml, nargo_version, run_nargo_test};
use crate::options::Options;
use crate::out;
use crate::project::Project;
use crate::report::{format_mutant_with_location, print_all_mutants, print_surviving_mutants};
use crate::run_report::{BaselineReport, MutationRunReport, RunSummary};
use crate::runner::run_all_mutants_in_temp;
use crate::scan::ProjectOverview;
use crate::ui::Ui;

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
    /// Show a project overview and mutation inventory summary.
    Scan {
        /// Path to the Noir project root or any path inside it.
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// List discovered mutants without executing tests.
    List {
        /// Path to the Noir project root or any path inside it.
        #[arg(long, default_value = ".")]
        project: PathBuf,

        /// Show only the first N discovered mutants (deterministic order).
        #[arg(long)]
        limit: Option<usize>,

        /// Emit a machine-readable JSON report to stdout.
        #[arg(long)]
        json: bool,

        /// Where to write discovery artifacts (writes `mutants.json` when set).
        #[arg(long)]
        out_dir: Option<PathBuf>,
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

        /// Where to write run artifacts (defaults to <project_root>/mutants.out).
        #[arg(long)]
        out_dir: Option<PathBuf>,
    },
}

fn print_json_and_exit(report: MutationRunReport, exit_code: i32) -> ! {
    let json = serde_json::to_string_pretty(&report).expect("serialize report to json");
    println!("{json}");
    std::process::exit(exit_code);
}

fn old_dir_for(out_dir: &Path) -> PathBuf {
    let parent = out_dir.parent().unwrap_or_else(|| Path::new("."));
    let name = out_dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "mutants.out".to_string());
    parent.join(format!("{name}.old"))
}

fn prepare_out_dir(out_dir: &Path) -> Result<()> {
    let old = old_dir_for(out_dir);

    if out_dir.exists() {
        if old.exists() {
            fs::remove_dir_all(&old).with_context(|| format!("failed to remove {:?}", old))?;
        }
        fs::rename(out_dir, &old)
            .with_context(|| format!("failed to rotate output dir {:?} -> {:?}", out_dir, old))?;
    }

    fs::create_dir_all(out_dir).with_context(|| format!("failed to create {:?}", out_dir))?;
    Ok(())
}

fn write_run_json(out_dir: &Path, report: &MutationRunReport) -> Result<()> {
    let path = out_dir.join("run.json");
    let json = serde_json::to_string_pretty(report).context("serialize report to json")?;
    fs::write(&path, json).with_context(|| format!("failed to write {:?}", path))?;
    Ok(())
}

#[derive(Debug, Serialize)]
struct MutationListReport {
    tool: &'static str,
    version: &'static str,
    project_root: PathBuf,
    discovered: usize,
    listed: usize,
    mutants: Vec<Mutant>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Default, Clone)]
struct ToolchainInfo {
    compiler_version: Option<String>,
    nargo_version: Option<String>,
}

/// Print toolchain context (copy/paste friendly) for diagnosing version mismatches.
/// Only used for non-JSON output.
fn print_toolchain_info(ui: &Ui, project_root: &Path) -> ToolchainInfo {
    let compiler_version = match compiler_version_from_nargo_toml(project_root) {
        Ok(v) => v,
        Err(e) => {
            ui.warn(format!("compiler_version (Nargo.toml): <error: {e}>"));
            None
        }
    };

    ui.line(format!(
        "compiler_version (Nargo.toml): {}",
        compiler_version.as_deref().unwrap_or("<none>")
    ));

    let nargo_v = match nargo_version() {
        Ok(v) => Some(v),
        Err(e) => {
            ui.warn(format!("nargo --version: <error: {e}>"));
            None
        }
    };

    if let Some(v) = nargo_v.as_deref() {
        ui.line(format!("nargo --version: {v}"));
    }

    ToolchainInfo {
        compiler_version,
        nargo_version: nargo_v,
    }
}

fn print_baseline_failure_hint(ui: &Ui, toolchain: &ToolchainInfo) {
    ui.warn(
        "hint: baseline `nargo test` failures are often caused by a Noir/Nargo toolchain mismatch.",
    );

    if let Some(v) = toolchain.compiler_version.as_deref() {
        ui.warn(format!("hint: project compiler_version (Nargo.toml): {v}"));
    } else {
        ui.warn("hint: project compiler_version (Nargo.toml): <none>".to_string());
    }

    if let Some(v) = toolchain.nargo_version.as_deref() {
        ui.warn(format!("hint: your `nargo --version`: {v}"));
    } else {
        ui.warn("hint: your `nargo --version`: <unavailable>".to_string());
    }

    ui.warn("hint: try using the project's pinned toolchain (or align your Noir/Nargo version) and re-run."
        .to_string());
}

/// Parse CLI arguments and dispatch the selected command.
pub fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Scan { project } => {
            let ui = Ui::new(false);
            let options = Options::new(project);

            ui.title("zk-mutant: scan");
            ui.line(format!("project: {:?}", options.project_root));

            let project = match Project::from_root(options.project_root.clone()) {
                Ok(p) => p,
                Err(e) => {
                    ui.error(format!(
                        "failed to load Noir project at {:?}: {e}",
                        options.project_root
                    ));
                    return Err(e);
                }
            };

            let overview = ProjectOverview::from_project(&project);
            print_scan_summary(&overview, &ui);

            let mutants = discover_mutants(&project);
            print_mutation_inventory(&mutants, &ui);

            Ok(())
        }

        Command::List {
            project,
            limit,
            json,
            out_dir,
        } => {
            let ui = Ui::new(json);
            let options = Options::new(project);
            let project_root = options.project_root.clone();

            ui.title("zk-mutant: list");
            ui.line(format!("project: {:?}", project_root));

            // toolchain/version awareness (non-JSON output only).
            if !json {
                let _toolchain = print_toolchain_info(&ui, &project_root);
            }

            let project = match Project::from_root(project_root.clone()) {
                Ok(p) => p,
                Err(e) => {
                    let report = MutationListReport {
                        tool: "zk-mutant",
                        version: env!("CARGO_PKG_VERSION"),
                        project_root: project_root.clone(),
                        discovered: 0,
                        listed: 0,
                        mutants: Vec::new(),
                        error: Some(format!("failed to load Noir project: {e}")),
                    };

                    if json {
                        let txt =
                            serde_json::to_string_pretty(&report).expect("serialize list report");
                        println!("{txt}");
                        std::process::exit(EXIT_ERROR);
                    }

                    ui.error(format!(
                        "failed to load Noir project at {:?}: {e}",
                        project_root
                    ));
                    return Err(e);
                }
            };

            let discovered_mutants = discover_mutants(&project);
            let discovered = discovered_mutants.len();

            let mut listed_mutants = discovered_mutants.clone();
            if let Some(limit) = limit {
                if limit == 0 {
                    listed_mutants.clear();
                } else if listed_mutants.len() > limit {
                    listed_mutants.truncate(limit);
                }
            }

            if let Some(out_dir) = out_dir.as_ref() {
                if let Err(e) = prepare_out_dir(out_dir) {
                    let report = MutationListReport {
                        tool: "zk-mutant",
                        version: env!("CARGO_PKG_VERSION"),
                        project_root: project_root.clone(),
                        discovered,
                        listed: listed_mutants.len(),
                        mutants: Vec::new(),
                        error: Some(format!("failed to prepare out dir {:?}: {e}", out_dir)),
                    };

                    if json {
                        let txt =
                            serde_json::to_string_pretty(&report).expect("serialize list report");
                        println!("{txt}");
                        std::process::exit(EXIT_ERROR);
                    }

                    ui.error(format!("failed to prepare out dir {:?}: {e}", out_dir));
                    return Err(e);
                }

                if let Err(e) = out::write_mutants_json(out_dir, &discovered_mutants) {
                    ui.warn(format!("failed to write mutants.json: {e}"));
                }
            }

            if json {
                let report = MutationListReport {
                    tool: "zk-mutant",
                    version: env!("CARGO_PKG_VERSION"),
                    project_root: project_root.clone(),
                    discovered,
                    listed: listed_mutants.len(),
                    mutants: listed_mutants,
                    error: None,
                };
                let txt = serde_json::to_string_pretty(&report).expect("serialize list report");
                println!("{txt}");
                std::process::exit(EXIT_OK);
            }

            ui.line(format!("discovered {} mutants", discovered));
            if let Some(limit) = limit {
                ui.line(format!(
                    "listed {} mutants (limit: {})",
                    listed_mutants.len(),
                    limit
                ));
            } else {
                ui.line(format!("listed {} mutants", listed_mutants.len()));
            }

            ui.line("--- mutants (discovered) ---");
            for m in &listed_mutants {
                ui.line(format_mutant_with_location(&project, m));
            }

            Ok(())
        }

        Command::Run {
            project,
            verbose,
            limit,
            json,
            fail_on_survivors,
            out_dir,
        } => {
            let ui = Ui::new(json);
            let options = Options::new(project);
            let project_root = options.project_root.clone();

            // Output directory (rotate + create)
            let out_dir = out_dir.unwrap_or_else(|| project_root.join("mutants.out"));
            if let Err(e) = prepare_out_dir(&out_dir) {
                if json {
                    let report = MutationRunReport::failure(
                        project_root.clone(),
                        BaselineReport {
                            success: false,
                            exit_code: None,
                            duration_ms: 0,
                        },
                        format!("failed to prepare out dir {:?}: {e}", out_dir),
                    );
                    print_json_and_exit(report, EXIT_ERROR);
                }
                ui.error(format!("failed to prepare out dir {:?}: {e}", out_dir));
                return Err(e);
            }

            ui.title("zk-mutant: run");
            ui.line(format!("project: {:?}", project_root));

            // toolchain/version awareness (non-JSON output only).
            let toolchain = if !json {
                print_toolchain_info(&ui, &project_root)
            } else {
                ToolchainInfo::default()
            };

            // Load Noir project and metrics via noir-metrics.
            let project = match Project::from_root(project_root.clone()) {
                Ok(p) => p,
                Err(e) => {
                    let report = MutationRunReport::failure(
                        project_root.clone(),
                        BaselineReport {
                            success: false,
                            exit_code: None,
                            duration_ms: 0,
                        },
                        format!("failed to load Noir project: {e}"),
                    );
                    let _ = write_run_json(&out_dir, &report);

                    if json {
                        print_json_and_exit(report, EXIT_ERROR);
                    }

                    ui.error(format!(
                        "failed to load Noir project at {:?}: {e}",
                        project_root
                    ));
                    return Err(e);
                }
            };

            // Baseline `nargo test` run before mutation testing.
            let baseline_result = match run_nargo_test(project.root()) {
                Ok(r) => r,
                Err(e) => {
                    let report = MutationRunReport::failure(
                        project_root.clone(),
                        BaselineReport {
                            success: false,
                            exit_code: None,
                            duration_ms: 0,
                        },
                        format!("failed to run `nargo test`: {e}"),
                    );
                    let _ = write_run_json(&out_dir, &report);

                    if json {
                        print_json_and_exit(report, EXIT_ERROR);
                    }

                    ui.error(format!(
                        "failed to run `nargo test` in {:?}: {e}",
                        project.root()
                    ));
                    return Err(e);
                }
            };

            let baseline = BaselineReport::from_nargo(&baseline_result);

            ui.line(format!(
                "nargo test finished in {:?} (exit code: {:?}, success: {})",
                baseline_result.duration, baseline_result.exit_code, baseline_result.success
            ));

            if !baseline_result.success {
                let report = MutationRunReport::failure(
                    project_root.clone(),
                    baseline,
                    "baseline `nargo test` failed".to_string(),
                );
                let _ = write_run_json(&out_dir, &report);

                if json {
                    print_json_and_exit(report, EXIT_ERROR);
                }

                ui.error("nargo test failed");

                if !baseline_result.stdout.is_empty() {
                    ui.error(format!("stdout from nargo:\n{}", baseline_result.stdout));
                }
                if !baseline_result.stderr.is_empty() {
                    ui.error(format!("stderr from nargo:\n{}", baseline_result.stderr));
                }

                // Helpful hint for likely version mismatch.
                print_baseline_failure_hint(&ui, &toolchain);

                return Err(anyhow::anyhow!("baseline `nargo test` failed"));
            }

            // Discover mutation opportunities.
            let mut mutants = discover_mutants(&project);
            let discovered = mutants.len();

            // Persist discovery list (pre-limit) as mutants.json
            if let Err(e) = out::write_mutants_json(&out_dir, &mutants) {
                ui.warn(format!("failed to write mutants.json: {e}"));
            }

            ui.line(format!("discovered {} mutants", discovered));

            if discovered == 0 {
                let report = MutationRunReport::success(
                    project_root.clone(),
                    0,
                    0,
                    baseline,
                    RunSummary::default(),
                    Vec::new(),
                );
                let _ = write_run_json(&out_dir, &report);

                if json {
                    print_json_and_exit(report, EXIT_OK);
                }

                ui.line("no mutants discovered, exiting");
                return Ok(());
            }

            if let Some(limit) = limit {
                if limit == 0 {
                    let report = MutationRunReport::success(
                        project_root.clone(),
                        discovered,
                        0,
                        baseline,
                        RunSummary::default(),
                        Vec::new(),
                    );
                    let _ = write_run_json(&out_dir, &report);

                    if json {
                        print_json_and_exit(report, EXIT_OK);
                    }

                    ui.line("mutant limit is 0, exiting");
                    return Ok(());
                }

                if mutants.len() > limit {
                    mutants.truncate(limit);
                }

                ui.line(format!(
                    "running {} mutants (of {})",
                    mutants.len(),
                    discovered
                ));
            }

            // Run all mutants sequentially (naive implementation).
            let executed = mutants.len();
            let summary = run_all_mutants_in_temp(&project, &mut mutants, &ui)?;

            // CI policy
            let wants_ci_fail = fail_on_survivors && summary.survived > 0;
            let exit_code = if wants_ci_fail {
                EXIT_SURVIVORS
            } else {
                EXIT_OK
            };

            let report = MutationRunReport::success(
                project_root.clone(),
                discovered,
                executed,
                baseline,
                summary,
                mutants,
            );

            // Always persist report to mutants.out/run.json
            let _ = write_run_json(&out_dir, &report);

            if let Err(e) = out::write_outcomes_json(&out_dir, &report) {
                ui.warn(format!("failed to write outcomes.json: {e}"));
            }

            if let Err(e) = out::write_outcome_txts(&out_dir, &project, &report.mutants) {
                ui.warn(format!("failed to write outcome txt files: {e}"));
            }

            if let Err(e) = out::write_diff_dir(&out_dir, &report.mutants) {
                ui.warn(format!("failed to write diff dir: {e}"));
            }

            if let Err(e) = out::write_log(&out_dir, &report) {
                ui.warn(format!("failed to write log: {e}"));
            }

            if json {
                print_json_and_exit(report, exit_code);
            }

            ui.line("--- mutation run summary ---");
            ui.line(format!("mutants total:    {}", executed));
            ui.line(format!("mutants killed:   {}", report.summary.killed));
            ui.line(format!("mutants survived: {}", report.summary.survived));
            ui.line(format!("mutants invalid:  {}", report.summary.invalid));

            if verbose {
                print_all_mutants(&project, &report.mutants);
            }

            print_surviving_mutants(&project, &report.mutants);

            if wants_ci_fail {
                ui.error(format!(
                    "mutation testing failed policy: {} mutant(s) survived (--fail-on-survivors)",
                    report.summary.survived
                ));
                std::process::exit(EXIT_SURVIVORS);
            }

            Ok(())
        }
    }
}

fn print_mutation_inventory(mutants: &[Mutant], ui: &Ui) {
    ui.line("--- mutation inventory ---");
    ui.line(format!("discovered mutants: {}", mutants.len()));

    if mutants.is_empty() {
        ui.line("no mutation opportunities found");
        return;
    }

    let mut by_operator: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_category: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_file: BTreeMap<String, usize> = BTreeMap::new();

    for m in mutants {
        let op = format!("{:?}/{}", m.operator.category, m.operator.name);
        *by_operator.entry(op).or_insert(0) += 1;

        let cat = format!("{:?}", m.operator.category);
        *by_category.entry(cat).or_insert(0) += 1;

        let file = m.span.file.display().to_string();
        *by_file.entry(file).or_insert(0) += 1;
    }

    ui.line(format!("unique operators: {}", by_operator.len()));

    ui.line("by category:");
    for (cat, count) in by_category {
        ui.line(format!("  {cat}: {count}"));
    }

    ui.line("by operator:");
    for (op, count) in by_operator {
        ui.line(format!("  {op}: {count}"));
    }

    let mut files: Vec<(String, usize)> = by_file.into_iter().collect();
    files.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    ui.line("top files:");
    for (file, count) in files.into_iter().take(10) {
        ui.line(format!("  {file}: {count}"));
    }
}

/// Print a short summary based on the project overview.
fn print_scan_summary(overview: &ProjectOverview, ui: &Ui) {
    ui.line("--- project overview ---");
    ui.line(format!(
        "project root:            {}",
        overview.root.display()
    ));
    ui.line(format!("nr files (.nr):          {}", overview.nr_files));
    ui.line(format!("test files:              {}", overview.test_files));
    ui.line(format!("code lines:              {}", overview.code_lines));
    ui.line(format!(
        "test functions:          {}",
        overview.test_functions
    ));
    ui.line(format!("test code lines:         {}", overview.test_lines));
    ui.line(format!(
        "non-test code lines:     {}",
        overview.non_test_lines
    ));
    ui.line(format!(
        "test code ratio:         {:.2}% (test_lines / code_lines)",
        overview.test_code_ratio
    ));
}
