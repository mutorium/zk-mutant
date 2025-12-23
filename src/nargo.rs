use std::fs;
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

/// Run `nargo --version` and return a single-line string (copy/paste friendly).
pub fn nargo_version() -> Result<String> {
    let out = Command::new("nargo")
        .arg("--version")
        .output()
        .context("failed to execute `nargo --version`")?;

    let text = if out.stdout.is_empty() {
        String::from_utf8_lossy(&out.stderr).to_string()
    } else {
        String::from_utf8_lossy(&out.stdout).to_string()
    };

    let one_line = text.trim().replace('\n', " ");
    if !out.status.success() {
        anyhow::bail!("`nargo --version` failed: {one_line}");
    }

    Ok(one_line)
}

/// Read `compiler_version = "..."` from `Nargo.toml` if present.
pub fn compiler_version_from_nargo_toml(project_root: &Path) -> Result<Option<String>> {
    let path = project_root.join("Nargo.toml");
    let contents = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).with_context(|| format!("failed to read {:?}", path)),
    };

    for raw_line in contents.lines() {
        // Strip inline comments.
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if !line.starts_with("compiler_version") {
            continue;
        }

        let rhs = match line.split_once('=') {
            Some((_, rhs)) => rhs.trim(),
            None => continue,
        };

        // Accept "0.x.y" or '0.x.y'
        let v = rhs.trim().trim_matches('"').trim_matches('\'').trim();
        if !v.is_empty() {
            return Ok(Some(v.to_string()));
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn mk_temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let dir = std::env::temp_dir().join(format!("zk-mutant-nargo-test-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn compiler_version_none_when_missing_file() {
        let dir = mk_temp_dir();
        let v = compiler_version_from_nargo_toml(&dir).unwrap();
        assert_eq!(v, None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn compiler_version_parses_value_with_quotes() {
        let dir = mk_temp_dir();
        fs::write(dir.join("Nargo.toml"), r#"compiler_version = "0.35.0""#).unwrap();

        let v = compiler_version_from_nargo_toml(&dir).unwrap();
        assert_eq!(v.as_deref(), Some("0.35.0"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn compiler_version_ignores_inline_comments() {
        let dir = mk_temp_dir();
        fs::write(
            dir.join("Nargo.toml"),
            r#"compiler_version = "0.12.0" # pin"#,
        )
        .unwrap();

        let v = compiler_version_from_nargo_toml(&dir).unwrap();
        assert_eq!(v.as_deref(), Some("0.12.0"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn compiler_version_errors_when_nargo_toml_is_a_directory() {
        let dir = mk_temp_dir();

        // Make "Nargo.toml" a directory so read_to_string returns a non-NotFound error.
        fs::create_dir_all(dir.join("Nargo.toml")).unwrap();

        let err = compiler_version_from_nargo_toml(&dir).unwrap_err();
        assert!(err.to_string().contains("failed to read"));

        let _ = fs::remove_dir_all(&dir);
    }
}
