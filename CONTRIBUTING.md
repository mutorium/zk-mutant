# Contributing

Thanks for considering contributing!

## Development setup

- Install Rust stable
- Ensure `nargo` is installed and on your `PATH`

## Quality gates

Before opening a PR:

```bash
cargo fmt
cargo clippy --all-targets --all-features
cargo test
```

## Testing philosophy

- Prefer unit tests for pure functions.
- Prefer **insta snapshots** when output stability matters (CLI/report/JSON).
- Integration tests should avoid depending on a real Noir toolchain when possible (use the fake `nargo` approach).

## Commit style

Conventional commits are welcome, e.g.:

- `docs: add roadmap`
- `test: add CLI integration snapshots`
- `feat: add json output`
- `refactor: simplify report rendering`
