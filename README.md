# zk-mutant

Mutation testing for [Noir](https://noir-lang.org/) circuits.

`zk-mutant` runs your Noir test suite under small **source-level mutations** (for example `== → !=`, `< → >=`) to estimate how well your tests catch bugs. If a mutation *survives*, your tests didn’t notice a change that might represent a real defect.

> **Status:** This is an early, fast-moving tool. The CLI surface, JSON formats, and output artifacts may evolve.
> Expect breaking changes before `1.0.0`.

---

## What it does

On `run`, `zk-mutant`:

1. Locates a Noir/Nargo project (a directory containing `Nargo.toml`).
2. Runs a **baseline** `nargo test` (must pass before mutation testing starts).
3. Discovers mutation opportunities deterministically.
4. Executes each mutant by running `nargo test` in a temporary project copy with the mutant applied.
5. Prints a summary and lists any surviving mutants.
6. Writes run artifacts to an output directory (default: `./mutants.out`, rotated to `./mutants.out.old`).

---

## Prerequisites

- Rust (edition 2024)
- `nargo` available on your `PATH` (Noir toolchain)

Tip: if your baseline `nargo test` fails due to a toolchain mismatch, `zk-mutant` can print the project's `compiler_version` (from `Nargo.toml`) and your `nargo --version` to help diagnose it.

---

## Installation

### From source (recommended for now)

```bash
git clone <REPO_URL>
cd zk-mutant
cargo install --path .
```

### From crates.io

Once published:

```bash
cargo install zk-mutant
```

---

## CLI overview

```bash
zk-mutant --help
```

Commands:

- `scan` — project overview + mutation inventory summary
- `preflight` — toolchain + baseline diagnostics (copy/paste friendly)
- `list` — list discovered mutants (no execution)
- `run` — run mutation testing

---

## Quickstart

From this repo (uses the included fixture):

```bash
cargo build
cargo test

zk-mutant scan --project tests/fixtures/simple_noir
zk-mutant run  --project tests/fixtures/simple_noir
```

Run against your own Noir project:

```bash
zk-mutant preflight --project .
zk-mutant run --project .
```

Helpful flags:

- `--limit N` — run only the first `N` mutants (deterministic order)
- `-v / --verbose` — print detailed per-mutant outcome lines
- `--json` — emit a machine-readable JSON report to stdout (human output stays on stderr)
- `--fail-on-survivors` — exit with code `2` if any mutants survive (CI-friendly)
- `--out-dir PATH` — write artifacts to a chosen directory (defaults to `<project_root>/mutants.out`)

Example:

```bash
zk-mutant run --project . --limit 25 -v --fail-on-survivors
```

---

## Exit codes

- `0` — success (and, if `--fail-on-survivors` is set, no survivors)
- `1` — error (baseline failed, project load failed, etc.)
- `2` — survivors found and `--fail-on-survivors` was set

---

## Output artifacts

By default, `zk-mutant` writes to `<project_root>/mutants.out` and rotates any existing directory to `<project_root>/mutants.out.old`.

Artifacts:

- `run.json` — full run report (tool, version, baseline, summary, mutants, errors)
- `mutants.json` — discovered mutants (pre-limit)
- `outcomes.json` — compact outcomes list (IDs + spans + outcome + duration)
- `caught.txt` / `missed.txt` / `unviable.txt` — cargo-mutants-style outcome lists
- `diff/000001.diff` — minimal snippet diffs for executed mutants
- `log` — stable text log (no timestamps) with baseline + summary + error

---

## Determinism

`zk-mutant` aims to be deterministic:

- Mutants are discovered in a stable order (sorted by file + span).
- IDs are assigned `1..N` in that deterministic order.
- `--limit` truncates after ordering, so repeated runs are stable.

---

## Limitations (v0.1.x)

- **Naive execution model:** currently copies the whole project for each mutant (simple and correct, but slower).
- **Source-level operators only:** no ACIR/Brillig-level mutation yet.
- **No advanced filtering:** operator/category/file filters are planned.
- **No parallelism:** concurrency/performance work is on the roadmap.

---

## Development

Run unit + integration tests:

```bash
cargo test
```

Run CLI snapshot tests (insta):

```bash
cargo test --test cli_integration
cargo insta test
```

Run mutation testing against `zk-mutant` itself (meta!):

```bash
cargo mutants
```

---

## License

MIT — see [LICENSE](LICENSE).
