# Arabic AI Models text

## What it does

Arabic text for the AI Models page (Arabic arrived upstream after PR #8231; a block at the end of `ar.tsv`, commit on custom)

## Branch

custom

## Own files

none

## Hooks in official files

ui-egui `i18n/ar.tsv` (13 lines appended)

## Tests

`ai_models`

```bash
cargo test -p photocraft-ui-egui --lib -- ai_models
```

## After the craft UI merge (2026-11-27)

Works: the Arabic rows (and the Swedish ones) are in `fork/i18n/ar.tsv` and `sv.tsv`, with the new text of the rebuilt pages. Rebuild: nothing.
