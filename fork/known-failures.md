# Known failures on this Mac (not caused by the fork)

- `dialogs::palette::list::upgrade_tests::page_up_and_page_down_move_a_page_and_stop_at_the_ends` (fork) failed once in a 1000-test run on 2026-10-29 and passes alone: load-flaky.

Checked 2026-10-28 on official `main` (b8c3f3aa8) as well: the same failures, none in a fork file.

- `clippy -D warnings`: this Mac's clippy (1.96) is newer than upstream's and flags official code:
  `nonminimal_bool` (`codecs/src/codecs/via_image.rs`, `io/src/smart_map/import.rs`, ui-egui `menus/menu_keys.rs`, `menus/menu_nav.rs`), `overly_complex_bool_expr` (`text/src/psd_path.rs`),
  `drop_non_drop` (`io/src/psd_export/document.rs`), cognitive complexity (`apps/tests/mac_menu_appkit.rs`), `manual_range_contains` (`engine/src/canvas_geom/tests.rs`).
- 28 `photocraft-ui-egui` tests (`i18n::live_tests`, `camera_raw_localization`, `type_tool`, `layer_row_ui`,
  `brush_panel`, `vector_ui`, `input::hold_keys`): `No glyph for …`; this Mac has no CJK font installed.
- `cargo xtask scorecard --check`: the total of settings ("12 of N") is higher than the committed 181: the fork's own settings (Double Exposure button, Scroll Wheel Changes Blend Mode, Select Subject Engine) and the extra crates (`macvision`, `ml`) add to it; `docs/scorecard.md` is left as upstream's so updates don't clash on it. For the same reason the xtask test `scorecard::tests::staleness_check_and_determinism` fails ("docs/scorecard.md is stale").
- `photocraft-automation rpc::tests::tcp_connections_must_authenticate_before_the_deadline_whatever_they_send` can fail with `ConnectionRefused` when the Mac is heavily loaded (several builds at once); it passes when run alone.
- `cargo xtask ci` stops at its first failing step; run the rest by hand (`cmd_ci` in `xtask/src/main.rs` lists them).
- `prefs_ui::ai_models::tests::dynamic_labels_are_translated_in_complete_languages` (fork): "sv: missing AI Models": Swedish became a complete language upstream after the fork added the AI Models page; needs Swedish lines like the Arabic ones (`fork/arabic-ai-models.md`). Seen 2026-11-01.
