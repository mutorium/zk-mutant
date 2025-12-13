use std::path::PathBuf;

/// Configuration options for zk-mutant derived from the CLI.
#[derive(Debug, Clone)]
pub struct Options {
    /// Path to the Noir project root.
    pub project_root: PathBuf,
}

impl Options {
    /// Construct an `Options` instance with default values.
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }
}
