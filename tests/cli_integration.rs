use assert_cmd::Command;
use regex::Regex;
use serde_json::Value;
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

# Support version queries (zk-mutant prints this for toolchain awareness).
if [[ "${1-}" == "--version" || "${1-}" == "-V" || "${1-}" == "version" ]]; then
  echo "nargo 0.0.0-test"
  exit 0
fi

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
if "%1"=="--version" goto version
if "%1"=="-V" goto version
if "%1"=="version" goto version

if "%1"=="test" goto test
echo fake nargo: only 'test' supported 1>&2
exit /b 2

:version
echo nargo 0.0.0-test
exit /b 0

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

fn run_zk_mutant_with_out_dir(
    args: &[&str],
    envs: &[(&str, &str)],
    out_dir: &Path,
) -> std::process::Output {
    let fake_nargo = make_fake_nargo_dir();
    let new_path = prepend_path(fake_nargo.path());

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

    cmd.output().expect("command should run")
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
fn list_fixture_snapshot() {
    let out = run_zk_mutant(&["list", "--project", "tests/fixtures/simple_noir"], &[]);
    insta::assert_snapshot!("list_fixture", out);
}

#[test]
fn list_fixture_json_snapshot() {
    let out = run_zk_mutant_stdout(
        &["list", "--project", "tests/fixtures/simple_noir", "--json"],
        &[],
    );
    insta::assert_snapshot!("list_fixture_json", out);
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

#[test]
fn run_writes_out_dir_artifacts() {
    let out_td = TempDir::new().expect("TempDir for out-dir should create");
    let out_dir = out_td.path().join("mutants.out");

    let out = run_zk_mutant_with_out_dir(
        &[
            "run",
            "--project",
            "tests/fixtures/simple_noir",
            "--limit",
            "1",
            "--out-dir",
            &out_dir.to_string_lossy(),
        ],
        &[],
        &out_dir,
    );

    assert!(
        out.status.success(),
        "expected success, got: {:?}\nstdout:\n{}\nstderr:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );

    let must_exist = [
        "run.json",
        "mutants.json",
        "outcomes.json",
        "caught.txt",
        "missed.txt",
        "unviable.txt",
        "log",
    ];

    for rel in must_exist {
        let p = out_dir.join(rel);
        assert!(p.exists(), "expected {:?} to exist", p);
    }

    // diff/ should exist and contain at least one diff when at least one mutant executed.
    let diff_dir = out_dir.join("diff");
    assert!(diff_dir.exists(), "expected {:?} to exist", diff_dir);
    let diff_count = fs::read_dir(&diff_dir)
        .expect("read diff dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .count();
    assert!(diff_count > 0, "expected at least one diff file");

    // JSON files should parse.
    let run_json = fs::read_to_string(out_dir.join("run.json")).expect("read run.json");
    let _: Value = serde_json::from_str(&run_json).expect("run.json parses");

    let mutants_json = fs::read_to_string(out_dir.join("mutants.json")).expect("read mutants.json");
    let _: Value = serde_json::from_str(&mutants_json).expect("mutants.json parses");

    let outcomes_json =
        fs::read_to_string(out_dir.join("outcomes.json")).expect("read outcomes.json");
    let _: Value = serde_json::from_str(&outcomes_json).expect("outcomes.json parses");
}

#[test]
fn run_out_dir_rotates_to_old() {
    let out_td = TempDir::new().expect("TempDir for out-dir should create");
    let out_dir = out_td.path().join("mutants.out");

    // 1st run
    let out1 = run_zk_mutant_with_out_dir(
        &[
            "run",
            "--project",
            "tests/fixtures/simple_noir",
            "--limit",
            "1",
            "--out-dir",
            &out_dir.to_string_lossy(),
        ],
        &[],
        &out_dir,
    );
    assert!(out1.status.success(), "first run should succeed");
    assert!(
        out_dir.join("run.json").exists(),
        "run.json should exist after first run"
    );

    // 2nd run (should rotate to mutants.out.old)
    let out2 = run_zk_mutant_with_out_dir(
        &[
            "run",
            "--project",
            "tests/fixtures/simple_noir",
            "--limit",
            "1",
            "--out-dir",
            &out_dir.to_string_lossy(),
        ],
        &[],
        &out_dir,
    );
    assert!(out2.status.success(), "second run should succeed");

    let old_dir = out_td.path().join("mutants.out.old");
    assert!(old_dir.exists(), "expected {:?} to exist", old_dir);
    assert!(
        old_dir.join("run.json").exists(),
        "expected rotated run.json to exist"
    );
    assert!(
        out_dir.join("run.json").exists(),
        "expected new run.json to exist"
    );
}

#[test]
fn run_fail_on_survivors_limit_0_exit_code_is_0() {
    let out_td = TempDir::new().expect("TempDir for out-dir should create");
    let out_dir = out_td.path().join("mutants.out");

    let out = run_zk_mutant_with_out_dir(
        &[
            "run",
            "--project",
            "tests/fixtures/simple_noir",
            "--limit",
            "0",
            "--fail-on-survivors",
            "--out-dir",
            &out_dir.to_string_lossy(),
        ],
        &[],
        &out_dir,
    );

    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit code 0\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}
