# Changelog

All notable changes to this project will be documented in this file.

This project aims to follow Semantic Versioning. While <1.0, minor releases should remain backwards compatible when reasonable.

## [Unreleased]
Planned for: `0.1.1`

### Fixed
- Fix: do not generate mutants from line/block comments (e.g., // ===== and equations in comments).

## [0.1.0] - 2025-12-23

Initial public release.

### Highlights
- Mutation testing for Noir projects: discover source-level mutants and run `nargo test` under mutation to measure test-suite strength.
- Deterministic mutant discovery and stable IDs (sorted by file + span; IDs assigned `1..N`).
- Human-friendly output and a machine-readable `--json` mode (JSON to stdout; human output routed to stderr).
- Reproducible output artifacts written to `mutants.out/` with directory rotation to `mutants.out.old/`.

### Commands
- `scan` — project overview and mutation inventory summary.
- `list` — list discovered mutants (optionally write discovery artifacts).
- `run` — baseline + mutation test execution (supports `--limit`, `--verbose`, `--fail-on-survivors`, `--json`).
- `preflight` — toolchain + baseline diagnostics for debugging version mismatches.

### Output artifacts
`run` writes the following to the output directory (default: `<project_root>/mutants.out/`):

- `run.json`
- `mutants.json`
- `outcomes.json`
- `caught.txt`, `missed.txt`, `unviable.txt`
- `diff/*.diff`
- `log`
