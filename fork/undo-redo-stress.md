# Undo and redo stress test

## What it does

Undo/redo stress test: hundreds of random edits, undos and redos (65 commands, 8/16/32-bit RGB, Gray, CMYK) must restore exact states, never panic or hang; `-- --ignored` for the long run, `PHOTOCRAFT_UNDO_STRESS_*` env vars

## Branch

`test/undo-redo-stress`

## Own files

`crates/engine/tests/undo_redo_stress.rs`

## Hooks in official files

none

## Tests

`cargo test -p photocraft-engine --test undo_redo_stress`

```bash
cargo test -p photocraft-engine --test undo_redo_stress
```
