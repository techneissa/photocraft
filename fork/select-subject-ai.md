# AI Select Subject and Remove Subject

## What it does

AI Select Subject and Remove Subject: upstream PR #8231 taken early (a real merge of its head `09aad5020`: BiRefNet-lite and LaMa through the pure-Rust `tract`, models downloaded only from Preferences ▸ AI Models or `photocraft-cli models fetch`, fixed URLs, pinned SHA-256), joined with the macOS cutout into ONE preference `tools.selectSubjectEngine` = auto (macOS cutout → AI model if installed → Classic) / macos / model / classic; the same choice drives Remove Background and Remove Subject; `select.subject` is a background job; `sampleAllLayers` now defaults to false (the selected layer). Fork fixes: bilinear matte upscale, the model plan freed after every run, cancel or error (peak ~9 GB, idle ~2.2 GB), a total-RAM check (under 12 GB → Classic with a note).

## Branch

`feat/select-subject-ai` (on `feat/select-subject-macos`)

## Own files

from the PR: `crates/ml/` (fork adds `ram.rs`), `crates/engine/src/cmd/remove_subject_cmds.rs`, `apps/photocraft-cli/src/models.rs`, `crates/ui-egui/src/prefs/prefs_ui/ai_models.rs`, `docs/ml.md`, `docs/remove-subject.md`; fork: `crates/engine/examples/select_subject_quickmask.rs`

## Hooks in official files

engine `cmd.rs`, `lib.rs`, `cmd/smartselect_cmds.rs` (finder chain, job, cache, notes), `cmd/cutout_cmds.rs`, `cmd/smartselect_macos.rs`, `prefs.rs` (`SubjectEngine::Model`); ui-egui `jobs_ui.rs`, `tools/retouch_ui.rs`, `panels/options_bar.rs`, `prefs/prefs_ui.rs` + `save.rs`, `dialogs.rs`, `fields.rs`, `tests.rs`, 20 `i18n/*.tsv`; CLI `args.rs`, `lib.rs`; `Cargo.toml` (workspace, engine, ui-egui, CLI), `Cargo.lock`, `xtask/src/layers.rs` (`ml`), `ATTRIBUTION.md`, `docs/gaps.md`, `docs/parity-checklist.md`

## Tests

`cargo test -p photocraft-ml`, engine `smartselect cutout remove_subject`, ui-egui `retouch_ui prefs_ui jobs_ui`

```bash
cargo test -p photocraft-engine --lib -- smartselect cutout remove_subject
cargo test -p photocraft-ml
```

## When upstream merges

**When upstream merges #8231:** by merge commit, git sees it as merged; if squashed, expect conflicts in `smartselect_cmds.rs`, `cutout_cmds.rs`, `remove_subject_cmds.rs`, `ml/backend.rs`: keep upstream where it matches, re-apply the engine choice and the plan freeing. The merge commit `00f8aa768` does not name `jobs_ui.rs` (a `select.subject` result arm), the rewritten `smartselect_macos.rs` (`step`), `prefs_ui/fields.rs` (~90 lines: section list re-indent, AI Models line), nor `Cargo.lock`'s `itertools` 0.10.5 → 0.13/0.14/0.15 and `winsafe` `crates/ml/src/store.rs`, `install.rs` and `backend.rs` are rewritten by `feat/shared-models` (a model is a folder of `ModelFile`s, `model_location`, `install_dir`, `Source`): if #8231 lands as is, keep the fork's version of those three files, `lib.rs`'s `pub use store::{…}` line, `ai_models.rs` (`found`, `place`) and `models.rs` (`list`).
