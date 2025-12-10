use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{Context, Result};

/// Result of running `nargo test` in a Noir project.
#[derive(Debug)]
pub struct NargoTestResult {
    /// Exit code returned by `nargo` (if it exited normally).
    pub exit_code: Option<i32>,

    /// Did `nargo test` succeed (exit status 0)?
    pub success: bool,

    /// Captured standard output of the command.
    pub stdout: String,

    /// Captured standard error of the command.
    pub stderr: String,

    /// How long the command ran.
    pub duration: Duration,
}

/// Run `nargo test` in the given project directory.
pub fn run_nargo_test(project_root: &Path) -> Result<NargoTestResult> {
    let start = std::time::Instant::now();

    let output = Command::new("nargo")
        .arg("test")
        .current_dir(project_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("failed to run `nargo test` in {:?}", project_root))?;

    let duration = start.elapsed();

    let exit_code = output.status.code();
    let success = output.status.success();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    Ok(NargoTestResult {
        exit_code,
        success,
        stdout,
        stderr,
        duration,
    })
}
