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

    /// Access the underlying noir-metrics report.
    pub fn metrics(&self) -> &MetricsReport {
        &self.metrics
    }

    /// build `SourceFile` entries for all `.nr` files in the project.
    pub fn source_files(&self) -> Vec<SourceFile> {
        self.metrics
            .files
            .iter()
            .map(|fm| SourceFile::from_relative(&self.root, &fm.path))
            .collect()
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
}
