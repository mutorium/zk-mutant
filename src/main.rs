mod cli;
mod mutant;
mod nargo;
mod options;
mod scan;

/// Entry point for the `zk-mutant` binary.
fn main() -> anyhow::Result<()> {
    cli::run()
}
