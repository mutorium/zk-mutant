mod cli;
mod mutant;
mod nargo;
mod options;
mod project;
mod scan;
mod span;

/// Entry point for the `zk-mutant` binary.
fn main() -> anyhow::Result<()> {
    cli::run()
}
