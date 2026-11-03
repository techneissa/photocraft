# Lock the Background Layer setting

## What it does

Settings › General › Lock the Background Layer, off by default. Off, an opened JPEG or opaque PNG (and an opaque TIFF, WebP, raw and the like) is a normal unlocked "Layer 0", as a PNG with transparency already is, so any layer can go below it. On, it opens as the locked Background, as in the official app. A PSD, a native file and File › New are not covered: they keep their Background.

## Branch

`feat/background-lock-setting`

## Own files

`crates/io/src/fork_background_lock.rs`

## Hooks in official files

io `lib.rs` (`mod` line, `ImportOptions::lock_background`, two `apply` calls for flat and raw imports), engine `prefs.rs` (`general.lockBackgroundLayer`), `jobs.rs` (`import_options` passes it), `cmd/file_cmds/open.rs` (`file.openAs` reads with the user's import options), `cmd/file_cmds/tests.rs` (1 test appended), ui-egui `prefs/prefs_ui/fields.rs` (the label)

## Tests

`fork_background_lock`, `lock_background_layer_setting`

```bash
cargo test -p photocraft-io --lib fork_background_lock
cargo test -p photocraft-engine --lib lock_background_layer_setting
```
