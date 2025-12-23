# Changelog

All notable changes to this project will be documented in this file.

This project aims to follow Semantic Versioning. While <1.0, minor releases should remain backwards compatible when reasonable.

## [Unreleased] (target: 0.1.0)

### Added
- `run` command: discover mutants, run baseline `nargo test`, then run per-mutant tests.
- `--json` output mode (machine-readable JSON to stdout; human output routed to stderr).
- Output artifacts directory with rotation (`mutants.out/` and `mutants.out.old/`).
- Snapshot tests (insta) and integration tests using a fake `nargo`.
- Basic CLI UI abstraction with stable non-colored output under `NO_COLOR=1`.
- (internal) Add CLI integration snapshots for `list` command.
- Discovery: avoid generating overlapping single-character mutants inside `<=` and `>=`, and add a regression test to lock the behavior.
- Print project `compiler_version` (from Nargo.toml) and `nargo --version` in `list`/`run`, and show clearer baseline failure hints for likely toolchain mismatches.
- Add preflight subcommand for copy/paste-friendly toolchain + baseline diagnostics (optional JSON), and print compiler_version/nargo --version in list/run with a helpful mismatch hint on baseline failures.
- Tighten CLI/UI/output-path assertions and add coverage for edge cases uncovered by cargo-mutants
