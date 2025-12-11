use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Byte span inside a Noir source file.
///
/// Offsets are byte indices into the file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceSpan {
    /// Path to the source file (absolute or project-relative).
    pub file: PathBuf,

    /// Start byte offset (inclusive).
    pub start: u32,

    /// End byte offset (exclusive).
    pub end: u32,
}
