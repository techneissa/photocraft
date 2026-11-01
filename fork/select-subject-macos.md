# Select Subject on macOS

## What it does

Select › Subject with macOS's own subject cutout (Apple Vision `VNGenerateForegroundInstanceMaskRequest`, people segmentation on macOS 12–13): the whole person in about 0.1 s, soft edges kept. Preferences ▸ Tools ▸ Select Subject Engine: Automatic (default: macOS on a Mac, else Classic), macOS, Classic; a fallback shows a short status note. Select and Mask's Subject button and Remove Background still use Classic. `crates/macvision` is the fork's second crate allowed `unsafe`

## Branch

`feat/select-subject-macos`

## Own files

`crates/macvision/` (`lib.rs`, `macos.rs`, `scale.rs`, `stub.rs`, README, tests), `crates/engine/src/cmd/smartselect_macos.rs` and `tests.rs`, `crates/engine/examples/bench_select_subject_engines.rs`

## Hooks in official files

`Cargo.toml` (workspace row), `Cargo.lock` (new entries, `objc2-vision` 0.3.2), `crates/engine/Cargo.toml`, `xtask/src/layers.rs` (Standalone), `AGENTS.md` (one sentence in "No unsafe"), engine `cmd.rs` (module), `prefs.rs` (`SubjectEngine`, `tools.select_subject_engine`), `prefs/validate.rs` (one arm), `cmd/smartselect_cmds.rs` (`SubjectCache` engine + note, `select_subject` calls the module, `note` in the result); ui-egui `prefs/prefs_ui/fields.rs` (labels), `tools/retouch_ui.rs` (`report_smart_result` shows the note), `menus/invoke.rs` (`select.subject` arm reports to the status bar)

## Tests

`cargo test -p photocraft-macvision`, engine `smartselect` (Mac tests run real Vision; `PHOTOCRAFT_MACVISION_IMAGE` = a portrait for the extra one)

```bash
cargo test -p photocraft-macvision
cargo test -p photocraft-engine --lib -- smartselect cutout remove_subject
```
