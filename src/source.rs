use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

/// Noir source file within a project
#[derive(Debug, Clone)]
pub struct SourceFile {
    /// Path relative to the project root (for example `src/main.nr`).
    pub root_relative: PathBuf,

    /// Absoulte path on disk.
    pub absoulete_path: PathBuf,
}

impl SourceFile {
    /// Construct a `SourceFile` from a project root and a relative path.
    pub fn from_relative(root: &Path, rel: &Path) -> Self {
        let absoulete_path = root.join(rel);
        Self {
            root_relative: rel.to_path_buf(),
            absoulete_path,
        }
    }

    /// Absolute path on disk
    pub fn path(&self) -> &Path {
        &self.absoulete_path
    }

    /// Path relative to the project root
    pub fn relative_path(&self) -> &Path {
        &self.root_relative
    }

    /// Load the full file contents as UTF-8 text.
    pub fn read_to_string(&self) -> Result<String> {
        let contents = fs::read_to_string(&self.absoulete_path)?;
        Ok(contents)
    }
}
