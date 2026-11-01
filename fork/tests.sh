#!/bin/sh
# Every feature's tests, run after a merge (see FORK.md). Starts from the repo root.
set -e
cd "$(dirname "$0")/.."
cargo test -p photocraft-engine --lib -- open_as_layers double_exposure
cargo test -p photocraft-ui-egui --lib -- open_as_layers double_exposure blend_wheel blend_dropdown options_bar_mode_wheel dropdown_search type_ahead hold_keys transform_tool:: transform_live puppet_ui perspective_ui transform_undo_tests guides_draw distort_ui vp_ui blend_preview
cargo test -p photocraft-compose --lib live
cargo test -p photocraft-macvision
cargo test -p photocraft-engine --lib -- smartselect cutout remove_subject
cargo test -p photocraft-ml
cargo test -p photocraft-text --lib -- big_collection
cargo test -p photocraft-engine --lib -- when chords chord_cmds shortcut_backup shortcut_sets image_cmds mode_cmds
cargo test -p photocraft-engine --test undo_redo_stress
cargo test -p photocraft-ui-egui --lib -- close_keys start_screen layer_shortcut native_menu title_bar_items titlebar assign_shortcut dialogs::palette menu_custom canvas_tool_menu layer_menu tab_context keybinds shortcuts_upgrade shortcut_capture shortcut_set
cargo test -p photocraft-gpu --test it live_transform
cargo test -p photocraft-ui-egui --lib -- ai_models
cargo test -p photocraft-cli -- models
cargo test -p photocraft-engine --lib -- distort_cmds lens_cmds channel_cmds targeting vp_cmds edit_cmds clipboard
cargo test -p photocraft-ui-egui --lib -- liquify_ui rasterize_prompt distort_ui camera_raw mask_clipboard polygon_lasso lasso_ui filter_dialog
cargo test -p photocraft-engine --lib -- layer_copy_cmds
cargo test -p photocraft-engine --test it -- smart_clipboard_sources
cargo test -p photocraft-ui-egui --lib -- duplicate_layer_ui
