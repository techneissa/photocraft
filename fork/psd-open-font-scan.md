# PSD font scan

## What it does

PSD opens no longer stall on the font scan (upstream issue #9624): fontique registers a font collection with K faces once per face (cost ~K⁴; 400 faces took minutes); a collection with more than 32 faces is now read once (at most 256 MiB, 512 MiB in total) and registered from memory, any error or cap falls back to the old path route with a log line

## Branch

`fix/psd-open-font-scan`

## Own files

`crates/text/src/fonts/big_collection.rs` and `big_collection/tests.rs`

## Hooks in official files

`crates/text/src/fonts.rs` (the `register_file` call, `mod` line), `crates/text/Cargo.toml` (`log`), `Cargo.lock`

## Tests

`cargo test -p photocraft-text --lib -- big_collection`

```bash
cargo test -p photocraft-text --lib -- big_collection
```
