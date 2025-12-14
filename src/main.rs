mod cli;
mod discover;
mod mutant;
mod nargo;
mod options;
mod out;
mod patch;
mod project;
mod report;
mod run_report;
mod runner;
mod scan;
mod source;
mod span;
mod ui;

/// Entry point for the `zk-mutant` binary.
fn main() -> anyhow::Result<()> {
    cli::run()
}
