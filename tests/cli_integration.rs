use assert_cmd::Command;
use regex::Regex;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn make_fake_nargo_dir() -> TempDir {
    let td = TempDir::new().expect("TempDir should create");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let nargo_path = td.path().join("nargo");
        let script = r#"#!/usr/bin/env bash
set -euo pipefail

if [[ "${1-}" != "test" ]]; then
  echo "fake nargo: only 'test' supported" >&2
  exit 2
fi

if [[ "${ZK_MUTANT_FAKE_NARGO_FAIL-}" == "1" ]]; then
  echo "fake nargo: failing as requested" >&2
  exit 1
fi

echo "fake nargo: ok"
exit 0
"#;

        fs::write(&nargo_path, script).expect("write fake nargo");
        let mut perms = fs::metadata(&nargo_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&nargo_path, perms).unwrap();
    }

    #[cfg(windows)]
    {
        let nargo_path = td.path().join("nargo.cmd");
        let script = r#"@echo off
if "%1"=="test" goto test
echo fake nargo: only 'test' supported 1>&2
exit /b 2

:test
if "%ZK_MUTANT_FAKE_NARGO_FAIL%"=="1" (
  echo fake nargo: failing as requested 1>&2
  exit /b 1
)

echo fake nargo: ok
exit /b 0
"#;
        fs::write(&nargo_path, script).expect("write fake nargo");
    }

    td
}

fn prepend_path(dir: &Path) -> OsString {
    let old = std::env::var_os("PATH").unwrap_or_default();

    std::env::join_paths(std::iter::once(dir.to_path_buf()).chain(std::env::split_paths(&old)))
        .expect("join PATH")
}

fn normalize_output(text: &str) -> String {
    // Redact durations like `261.502302ms`, `8s`, `234ms`.
    let re_dur = Regex::new(r"\b\d+(\.\d+)?(ns|us|µs|ms|s)\b").unwrap();
    let out = re_dur.replace_all(text, "<DUR>");

    // Redact JSON duration_ms fields (these vary run-to-run).
    let re_json_dur_ms = Regex::new(r#""duration_ms"\s*:\s*\d+"#).unwrap();
    let out = re_json_dur_ms.replace_all(&out, r#""duration_ms": 0"#);

    // Defensive: redact tmp-ish paths if they ever appear.
    let re_tmp_unix = Regex::new(r"/tmp/[^\s]+").unwrap();
    let out = re_tmp_unix.replace_all(&out, "<TMP>");

    out.to_string()
}

fn run_zk_mutant(args: &[&str], envs: &[(&str, &str)]) -> String {
    let fake_nargo = make_fake_nargo_dir();
    let new_path = prepend_path(fake_nargo.path());

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("zk-mutant"));
    cmd.args(args)
        .env("PATH", new_path)
        .env("NO_COLOR", "1")
        .env("RUST_BACKTRACE", "0");

    for (k, v) in envs {
        cmd.env(k, v);
    }

    let output = cmd.output().expect("command should run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let combined = format!(
        "status: {}\n--- stdout ---\n{}--- stderr ---\n{}",
        output.status, stdout, stderr
    );

    normalize_output(&combined)
}

#[test]
fn cli_help_snapshot() {
    let out = run_zk_mutant(&["--help"], &[]);
    insta::assert_snapshot!("cli_help", out);
}

#[test]
fn scan_fixture_snapshot() {
    let out = run_zk_mutant(&["scan", "--project", "tests/fixtures/simple_noir"], &[]);
    insta::assert_snapshot!("scan_fixture", out);
}

#[test]
fn run_limit_0_snapshot() {
    let out = run_zk_mutant(
        &[
            "run",
            "--project",
            "tests/fixtures/simple_noir",
            "--limit",
            "0",
        ],
        &[],
    );
    insta::assert_snapshot!("run_limit_0", out);
}

#[test]
fn run_limit_1_verbose_snapshot() {
    let out = run_zk_mutant(
        &[
            "run",
            "--project",
            "tests/fixtures/simple_noir",
            "--limit",
            "1",
            "-v",
        ],
        &[],
    );
    insta::assert_snapshot!("run_limit_1_verbose", out);
}

#[test]
fn run_baseline_fail_snapshot() {
    let out = run_zk_mutant(
        &[
            "run",
            "--project",
            "tests/fixtures/simple_noir",
            "--limit",
            "1",
        ],
        &[("ZK_MUTANT_FAKE_NARGO_FAIL", "1")],
    );
    insta::assert_snapshot!("run_baseline_fail", out);
}

#[test]
fn run_limit_0_json_snapshot() {
    let out = run_zk_mutant(
        &[
            "run",
            "--project",
            "tests/fixtures/simple_noir",
            "--limit",
            "0",
            "--json",
        ],
        &[],
    );
    insta::assert_snapshot!("run_limit_0_json", out);
}

#[test]
fn run_baseline_fail_json_snapshot() {
    let out = run_zk_mutant(
        &[
            "run",
            "--project",
            "tests/fixtures/simple_noir",
            "--limit",
            "1",
            "--json",
        ],
        &[("ZK_MUTANT_FAKE_NARGO_FAIL", "1")],
    );
    insta::assert_snapshot!("run_baseline_fail_json", out);
}
