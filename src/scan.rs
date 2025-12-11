use std::path::{Path, PathBuf};

use crate::project::Project;
use anyhow::Result;
use noir_metrics::MetricsReport;

/// High-level overview of a Noir project used by zk-mutant.
#[derive(Debug, Clone)]
pub struct ProjectOverview {
    /// Absolute path to the project root.
    pub root: PathBuf,

    /// Number of `.nr` files in the project.
    pub nr_files: usize,

    /// Number of files that are considered test files.
    pub test_files: usize,

    /// Total number of `#[test...]` functions across the project.
    pub test_functions: usize,

    /// Total code lines across all `.nr` files.
    pub code_lines: usize,

    /// Code lines inside `#[test...]` functions.
    pub test_lines: usize,

    /// Code lines outside tests.
    pub non_test_lines: usize,

    /// Share of code that lives in tests (test_lines / code_lines * 100).
    pub test_code_ratio: f64,
}

impl ProjectOverview {
    /// Build a `ProjectOverview` from a loaded Noir project.
    pub fn from_project(project: &Project) -> Self {
        let report: &MetricsReport = &project.metrics;

        let test_files = report.files.iter().filter(|f| f.is_test_file).count();

        ProjectOverview {
            root: project.root.clone(),
            nr_files: report.totals.files,
            test_files,
            test_functions: report.totals.test_functions,
            code_lines: report.totals.code_lines,
            test_lines: report.totals.test_lines,
            non_test_lines: report.totals.non_test_lines,
            test_code_ratio: report.totals.test_code_percentage,
        }
    }
}

/// Run noir-metrics on the given root and return a high-level overview.
pub fn scan_project(root: &Path) -> Result<ProjectOverview> {
    let project = Project::from_root(root.to_path_buf())?;
    Ok(ProjectOverview::from_project(&project))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn scan_simple_noir_fixture() {
        let root = PathBuf::from("tests/fixtures/simple_noir");
        let mut overview = scan_project(&root).expect("scan_project should succeed");

        overview.root = root;

        insta::assert_debug_snapshot!("scan_simple_noir", overview);
    }
}
