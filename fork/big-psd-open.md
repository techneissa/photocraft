# Big PSD open

## What it does

A big layered PSD opens, and shows a preview on the Home screen. The PSD pixel allowance (official: 2 GiB, the error "PSD import needs an estimated N bytes of pixel memory; limit is 2147483648 bytes") follows the machine on a 64-bit Mac: half the installed memory, never below 2 GiB (8 GB Mac: 4 GiB, 16 GB: 8 GiB, 24 GB: 12 GiB, 36 GB: 18 GiB, 48 GB: 24 GiB). A file past it still gets the same error. The Home grid shows a PSD or PSB by its thumbnail resource (1036, read from the start of the file only), else its merged image (8 or 16-bit RGB, up to 1 GiB decoded), before any layer import, and also for files over the 512 MB preview limit; a file with neither is imported as before

## Branch

`fix/big-psd-open`

## Own files

`crates/io/src/fork_psd_budget.rs`, `crates/io/src/fork_psd_preview.rs`

## Hooks in official files

`crates/io/src/lib.rs` (two `mod` lines; the `.min(DEFAULT_PSD_IMPORT_PIXEL_BYTES)` caps and the default option call `fork_psd_budget::psd_import_pixel_bytes()`), `crates/io/src/psd_import.rs` and `crates/io/src/flat.rs` (the same call), `crates/ui-egui/src/canvas/home_grid/thumbs.rs` (`psd_preview` in `decode`), `crates/ui-egui/src/canvas/home_grid/tests.rs` (one test)

## Tests

`cargo test -p photocraft-io --lib fork_ps` and `cargo test -p photocraft-ui-egui --lib psd_preview_is_the_merged`

```bash
cargo test -p photocraft-io --lib fork_ps
cargo test -p photocraft-ui-egui --lib psd_preview_is_the_merged
```
