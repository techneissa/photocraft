# Big PSD open

## What it does

A big layered PSD shows a preview on the Home screen. (Opening it is now official: the import budget follows the Mac's available memory, up to 8 GiB without asking and up to three quarters of it after an approval, `crates/io/src/memory.rs` `resolve_budget`. The fork's own allowance, half the installed memory, was removed on 2026-11-05: it overrode that budget and capped it at 2 GiB on Linux, where it failed the official memory tests.) The Home grid shows a PSD or PSB by its thumbnail resource (1036, read from the start of the file only), else its merged image (8 or 16-bit RGB, up to 1 GiB decoded), before any layer import, and also for files over the 512 MB preview limit; a file with neither is imported as before

## Branch

`fix/big-psd-open`

## Own files

`crates/io/src/fork_psd_preview.rs`

## Hooks in official files

`crates/io/src/lib.rs` (one `mod` line), `crates/ui-egui/src/canvas/home_grid/thumbs.rs` (`psd_preview` in `decode`), `crates/ui-egui/src/canvas/home_grid/tests.rs` (one test)

## Tests

`cargo test -p photocraft-io --lib fork_psd_preview` and `cargo test -p photocraft-ui-egui --lib psd_preview_is_the_merged`

```bash
cargo test -p photocraft-io --lib fork_psd_preview
cargo test -p photocraft-ui-egui --lib psd_preview_is_the_merged
```
