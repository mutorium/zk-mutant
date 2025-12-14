use std::path::PathBuf;

use serde::Serialize;

use crate::mutant::Mutant;
use crate::nargo::NargoTestResult;

/// Summary counts for a mutation-testing run.
#[derive(Debug, Default, Clone, Serialize)]
pub struct RunSummary {
    /// Number of mutants whose tests failed under mutation.
    pub killed: usize,

    /// Number of mutants for which tests still passed.
    pub survived: usize,

    /// Number of mutants that could not be built or executed.
    pub invalid: usize,
}

/// Baseline `nargo test` metadata.
#[derive(Debug, Clone, Serialize)]
pub struct BaselineReport {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
}

impl BaselineReport {
    pub fn from_nargo(result: &NargoTestResult) -> Self {
        Self {
            success: result.success,
            exit_code: result.exit_code,
            duration_ms: result.duration.as_millis() as u64,
        }
    }
}

/// Machine-readable report for a mutation test run.
///
/// In `--json` mode we print this to stdout as pretty JSON.
#[derive(Debug, Serialize)]
pub struct MutationRunReport {
    /// Tool name, stable across versions.
    pub tool: &'static str,

    /// Current crate version.
    pub version: &'static str,

    /// The project root used for this run.
    pub project_root: PathBuf,

    /// Number of mutants discovered before applying `--limit`.
    pub discovered: usize,

    /// Number of mutants actually executed (after `--limit`).
    pub executed: usize,

    /// Baseline `nargo test` result.
    pub baseline: BaselineReport,

    /// Summary of mutant outcomes.
    pub summary: RunSummary,

    /// Mutants with updated outcomes.
    pub mutants: Vec<Mutant>,

    /// Optional high-level error message (for example baseline failure).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl MutationRunReport {
    pub fn success(
        project_root: PathBuf,
        discovered: usize,
        executed: usize,
        baseline: BaselineReport,
        summary: RunSummary,
        mutants: Vec<Mutant>,
    ) -> Self {
        Self {
            tool: "zk-mutant",
            version: env!("CARGO_PKG_VERSION"),
            project_root,
            discovered,
            executed,
            baseline,
            summary,
            mutants,
            error: None,
        }
    }

    pub fn failure(project_root: PathBuf, baseline: BaselineReport, error: String) -> Self {
        Self {
            tool: "zk-mutant",
            version: env!("CARGO_PKG_VERSION"),
            project_root,
            discovered: 0,
            executed: 0,
            baseline,
            summary: RunSummary::default(),
            mutants: Vec::new(),
            error: Some(error),
        }
    }
}
