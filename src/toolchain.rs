use std::{fs, path::Path, process::Command};

use anyhow::{Context, Result};

pub fn compiler_version_from_nargo_toml(project_root: &Path) -> Result<Option<String>> {
    let path = project_root.join("Nargo.toml");
    let contents = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).with_context(|| format!("failed to read {:?}", path)),
    };

    for raw_line in contents.lines() {
        // Strip comments and whitespace.
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if !line.starts_with("compiler_version") {
            continue;
        }

        let rhs = match line.splitn(2, '=').nth(1) {
            Some(v) => v.trim(),
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
