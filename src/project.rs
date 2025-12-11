use std::path::{Path, PathBuf};

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
}
