use std::path::{Path, PathBuf};

use crate::source::SourceFile;
use anyhow::Result;
use noir_metrics::{MetricsReport, analyze_path};

/// Noir project with precomputed metrics from noir-metrics
#[derive(Debug, Clone)]
pub struct Project {
    /// Root directory of the Noir project.
    pub root: PathBuf,

    /// Metrics report produced by noir-metrics.
    pub metrics: MetricsReport,
}

impl Project {
    /// Load a project and compute metrics for all `.nr` files under `root`.
    pub fn from_root(root: PathBuf) -> Result<Self> {
        // Delegate discovery + metrics to noir-metrics.
        let metrics = analyze_path(&root)?;
        Ok(Self { root, metrics })
    }

    /// Root directory as a `Path`.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Build `SourceFile` entries for all `.nr` files in the project.
    pub fn source_files(&self) -> Vec<SourceFile> {
        self.metrics
            .files
            .iter()
            .map(|fm| SourceFile::from_relative(&self.root, &fm.path))
            .collect()
    }

    /// Look up a source file by its project-relative path (for example `src/main.nr`).
    pub fn find_source(&self, rel: &std::path::Path) -> Option<SourceFile> {
        self.metrics
            .files
            .iter()
            .find(|fm| fm.path == rel)
            .map(|fm| SourceFile::from_relative(&self.root, &fm.path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn source_files_match_metrics_and_exist() {
        let root = PathBuf::from("tests/fixtures/simple_noir");
        let project = Project::from_root(root).expect("Project::from_root should suceed");

        let sources = project.source_files();

        // Same number of entries as noir-metrics report.
        assert_eq!(sources.len(), project.metrics.files.len());

        // All reported source paths exist on disk
        for src in &sources {
            assert!(
                src.path().exists(),
                "source path should exist on disk: {:?}",
                src.path()
            );
        }
    }

    #[test]
    fn find_source_returns_expected_file() {
        let root = PathBuf::from("tests/fixtures/simple_noir");
        let project = Project::from_root(root.clone()).expect("Project::from_root should succeed");

        let rel = std::path::Path::new("src/main.nr");
        let src = project
            .find_source(rel)
            .expect("find_source should return Some for existing file");

        assert_eq!(src.relative_path(), rel);
        assert!(
            src.path().exists(),
            "absolute path should exist on disk: {:?}",
            src.path()
        );
    }
}
