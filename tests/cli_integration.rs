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

    // join_paths wants a single item type; split_paths yields PathBuf, so we use PathBuf everywhere.
    std::env::join_paths(std::iter::once(dir.to_path_buf()).chain(std::env::split_paths(&old)))
        .expect("join PATH")
}

fn normalize_output(text: &str) -> String {
    // Redact textual durations like `261.502302ms`, `8s`, `234ms`.
    let re_dur_text = Regex::new(r"\b\d+(\.\d+)?(ns|us|µs|ms|s)\b").unwrap();
    let out = re_dur_text.replace_all(text, "<DUR>");

    // Redact JSON numeric duration fields to stabilize snapshots.
    let re_dur_ms = Regex::new(r#""duration_ms"\s*:\s*\d+"#).unwrap();
    let out = re_dur_ms.replace_all(&out, r#""duration_ms": 0"#);

    // Defensive: redact tmp-ish paths if they ever appear.
    let re_tmp_unix = Regex::new(r"/tmp/[^\s]+").unwrap();
    let out = re_tmp_unix.replace_all(&out, "<TMP>");

    // Optional extra defense for Windows temp paths.
    let re_tmp_win = Regex::new(r"[A-Za-z]:\\[^\s]+\\Temp\\[^\s]+").unwrap();
    let out = re_tmp_win.replace_all(&out, "<TMP>");

    out.to_string()
}

fn has_flag(args: &[&str], flag: &str) -> bool {
    args.contains(&flag)
}

/// Combined output helper (stdout + stderr + status) for human-mode snapshots.
fn run_zk_mutant(args: &[&str], envs: &[(&str, &str)]) -> String {
    let fake_nargo = make_fake_nargo_dir();
    let new_path = prepend_path(fake_nargo.path());

    // Ensure we never create/rotate mutants.out inside fixtures or repo during tests.
    let out_td = TempDir::new().expect("TempDir for out-dir should create");
    let out_dir = out_td.path().join("mutants.out");
    let out_dir_str = out_dir.to_string_lossy().to_string();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("zk-mutant"));
    cmd.args(args)
        .env("PATH", new_path)
        .env("NO_COLOR", "1")
        .env("RUST_BACKTRACE", "0");

    if args.first() == Some(&"run") && !has_flag(args, "--out-dir") {
        cmd.args(["--out-dir", &out_dir_str]);
    }

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

/// Stdout-only helper for `--json` snapshots (stdout should be machine-readable JSON).
fn run_zk_mutant_stdout(args: &[&str], envs: &[(&str, &str)]) -> String {
    let fake_nargo = make_fake_nargo_dir();
    let new_path = prepend_path(fake_nargo.path());

    let out_td = TempDir::new().expect("TempDir for out-dir should create");
    let out_dir = out_td.path().join("mutants.out");
    let out_dir_str = out_dir.to_string_lossy().to_string();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("zk-mutant"));
    cmd.args(args)
        .env("PATH", new_path)
        .env("NO_COLOR", "1")
        .env("RUST_BACKTRACE", "0");

    if args.first() == Some(&"run") && !has_flag(args, "--out-dir") {
        cmd.args(["--out-dir", &out_dir_str]);
    }

    for (k, v) in envs {
        cmd.env(k, v);
    }

    let output = cmd.output().expect("command should run");
    let stdout = String::from_utf8_lossy(&output.stdout);

    normalize_output(&stdout)
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
    let out = run_zk_mutant_stdout(
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
    let out = run_zk_mutant_stdout(
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

#[test]
fn run_no_limit_json_snapshot() {
    let out = run_zk_mutant_stdout(
        &["run", "--project", "tests/fixtures/simple_noir", "--json"],
        &[],
    );
    insta::assert_snapshot!("run_no_limit_json", out);
}

#[test]
fn run_fail_on_survivors_exit_code_is_2() {
    let fake_nargo = make_fake_nargo_dir();
    let new_path = prepend_path(fake_nargo.path());

    let out_td = TempDir::new().expect("TempDir for out-dir should create");
    let out_dir = out_td.path().join("mutants.out");
    let out_dir_str = out_dir.to_string_lossy().to_string();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("zk-mutant"));
    cmd.args([
        "run",
        "--project",
        "tests/fixtures/simple_noir",
        "--limit",
        "1",
        "--fail-on-survivors",
        "--out-dir",
        &out_dir_str,
    ])
    .env("PATH", new_path)
    .env("NO_COLOR", "1")
    .env("RUST_BACKTRACE", "0");

    let out = cmd.output().expect("command should run");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn run_fail_on_survivors_json_snapshot() {
    let out = run_zk_mutant_stdout(
        &[
            "run",
            "--project",
            "tests/fixtures/simple_noir",
            "--limit",
            "1",
            "--json",
            "--fail-on-survivors",
        ],
        &[],
    );
    insta::assert_snapshot!("run_fail_on_survivors_json", out);
}
