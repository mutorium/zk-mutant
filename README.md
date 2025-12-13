# zk-mutant

Mutation testing for Noir circuits.

`zk-mutant` runs your Noir tests under small source-level mutations (e.g. `==` → `!=`) to estimate how good your test suite is at catching bugs.

## Prerequisites

- Rust (edition 2024)
- `nargo` available on your `PATH` (Noir toolchain)

## Quickstart

From the repo root:

```bash
cargo build
cargo test
```

Scan a project:

```bash
cargo run -- scan --project tests/fixtures/simple_noir
```

Run mutation testing:

```bash
cargo run -- run --project tests/fixtures/simple_noir
```

Helpful flags:

- `--limit N` — run only the first `N` mutants (deterministic order)
- `-v / --verbose` — print a detailed per-mutant list (killed/survived/invalid + duration)

Example:

```bash
cargo run -- run --project tests/fixtures/simple_noir --limit 10 -v
```

## Output

- A baseline `nargo test` run is executed first; if baseline tests fail, mutation testing stops.
- Mutants are discovered deterministically (sorted by file + span; IDs assigned 1..N).
- A summary is printed at the end and surviving mutants are listed.

## Development

Run the full test suite:

```bash
cargo test
```

Run integration tests (CLI snapshots):

```bash
cargo test --test cli_integration
```

Run mutation testing against zk-mutant itself (meta!):

```bash
cargo mutants
```

## License

MIT — see `LICENSE`.
