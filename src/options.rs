use std::path::PathBuf;

/// Configuration options for zk-mutant derived from the CLI
#[derive(Debug, Clone)]
pub struct Options {
    /// Path to the Noir project root.
    pub project_root: PathBuf,

    /// Optional limit for the number of mutants to execute.
    pub mutant_limit: Option<usize>,

    /// When true, emit JSON output instead of human-readable summary.
    pub json_output: bool,

    /// Command used to invoke `nargo`.
    pub nargo_cmd: String,
}

impl Options {
    /// Contruct an `Options` instance with default values.
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            mutant_limit: None,
            json_output: false,
            nargo_cmd: "nargo".to_string(),
        }
    }
}
