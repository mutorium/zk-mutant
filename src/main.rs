mod cli;
mod discover;
mod mutant;
mod nargo;
mod options;
mod patch;
mod project;
mod scan;
mod source;
mod span;

/// Entry point for the `zk-mutant` binary.
fn main() -> anyhow::Result<()> {
    cli::run()
}
