# Design Notes (WIP)

This doc is intentionally short and pragmatic: it explains how zk-mutant works *today* and the constraints it assumes.

## Core pipeline

1. Load Noir project via `noir-metrics` (file discovery + metrics).
2. Baseline: run `nargo test` on the original project.
3. Discover mutation sites (currently: textual scan for comparison operators, skipping `#[test]` bodies via a textual brace-depth heuristic).
4. For each mutant:
   - copy project to a temp directory
   - apply patch into the copied tree
   - run `nargo test` in the temp tree
   - classify outcome (killed/survived/invalid) and record duration
5. Print summary + reports.

## Determinism

- Mutants are sorted by `(file, start_offset)` and assigned IDs 1..N.
- Reports sort by mutant ID.

## Reporting philosophy

- Default output should be readable and compact.
- “Diff style” output is avoided for large blocks; prefer one-line descriptions + exact location.
