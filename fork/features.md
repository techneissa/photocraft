# The features

One line per feature, newest last; each links to its `fork/<slug>.md`. Feature branches do not edit this
file: the batch on `custom` adds each merged feature's line (FORK.md step 6), so pull requests never clash
here. GitHub's pull request merge ignores `.gitattributes`' `merge=union`; a local `git merge` keeps both
lines. Add a line at the end; never reorder or rewrap the others.

- [Open as Layers](open-as-layers.md): opens the photos of a LightCraft `.pcstack` file as the layers of one new document. Branch `feat/open-as-layers`.
- [Transform live blend](transform-live-blend.md): Free Transform, Warp, Puppet Warp and Perspective Warp show the moving layer with its blend mode, opacity and mask. Branch `feat/transform-live-blend`.
- [Double Exposure](double-exposure.md): Layer ▸ Double Exposure sets the top layer to Screen with a Reveal All mask in one undo step. Branch `feat/double-exposure`.
- [Blend list height and wheel](blend-list.md): blend mode lists are as tall as the window, and the wheel scrolls them by default (a preference makes it step the mode). Branch `feat/blend-list`.
- [Dropdown search](dropdown-search.md): typing in an open dropdown list narrows it. Branch `feat/dropdown-search`.
- [Select Subject on macOS](select-subject-macos.md): Select › Subject uses macOS's own subject cutout (Apple Vision). Branch `feat/select-subject-macos`.
- [AI Select Subject and Remove Subject](select-subject-ai.md): upstream AI models (BiRefNet-lite and LaMa) joined with the macOS cutout under one engine preference. Branch `feat/select-subject-ai`.
- [Shared models folder](shared-models.md): the AI models live in one folder shared by the Craft apps. Branch `feat/shared-models`.
- [Upstream select and mask fixes](upstream-select-mask.md): four upstream fixes taken early, for mask filters, mask copy and paste, the Polygonal Lasso and Select › Modify previews. Branch `feat/upstream-select-mask`.
- [Copy Layers to Document](duplicate-to-other-doc.md): refuses a destination with a running background job; two upstream PRs taken early; Duplicate Layer… says where the copy went ("Duplicated to …"). Branches `fix/duplicate-to-other-doc`, `feat/dup-note-protect-subject`.
- [PSD font scan](psd-open-font-scan.md): PSD files with a big font collection open without the long font scan. Branch `fix/psd-open-font-scan`.
- [Top bar items](topbar-items.md): show or hide the top bar's Logo, Document Title, Workspace, Search and Appearance Mode. Branch `feat/topbar-items`.
- [Assign Shortcut and command palette](assign-shortcut-palette.md): Assign Shortcut… on right-click, and the command palette on ⌘P as well as ⌘F. Branch `feat/assign-shortcut-palette`.
- [Context menu editor](context-menu-editor.md): arranges the right-click menus: hide, reorder and add commands, with presets and a backup file. Branch `feat/context-menu-editor`.
- [Keyboard shortcut upgrade](shortcut-upgrade.md): chords, When conditions, a clash icon and export/import icons in Keyboard Shortcuts. Branch `feat/shortcut-upgrade`.
- [Crop W×H](pr-9213.md): crop with Delete Cropped Pixels off no longer aborts the app (upstream PR #9213, taken early). Branch `pr-9213`.
- [Undo and redo stress test](undo-redo-stress.md): hundreds of random edits, undos and redos must restore exact states. Branch `test/undo-redo-stress`.
- [Discord button hidden](hide-discord-button.md): no Join us on Discord button on the start screen or in About. Branch `feat/hide-discord-button`.
- [⌘, opens Settings](settings-cmd-comma.md): ⌘, opens Settings, as in every Mac app; Hide Layers has no default key. Branch `feat/settings-cmd-comma`.
- [⌘, or Esc closes Preferences](settings-close-keys.md): closes Preferences, asking to save when there are unsaved changes. Branch `feat/settings-close-keys`.
- [Preferences scroll width](prefs-scroll-width.md): Preferences lists fill the window, the Toolbar tab fits 1–4 columns, Keyboard Shortcuts keeps its width while searching. Branch `fix/prefs-scroll-width`, `fix/shortcuts-tabs-layout`, `fix/shortcuts-search-width`.
- [Context menus page size](context-menus-resize.md): the Context Menus page keeps the size the Preferences window is dragged to. Branch `fix/context-menus-resize`.
- [Arabic AI Models text](arabic-ai-models.md): Arabic text for the AI Models page. Branch `custom`.
- [Place Photos](place-photos.md): Place Embedded on ⇧⌘P, several photos at once, a Place Photo button in the Layers panel. Branch `feat/place-photos`.
- [SAM 3](sam3.md): LightCraft's SAM 3 model code, click and text masks, no UI yet. Branch `feat/sam3`.
- [Refine Hair](refine-hair.md): Select and Mask's Refine Hair button, closed-form matting in the edge band; a Protect Subject box keeps the subject SAM 3 finds. Branches `feat/refine-hair`, `feat/dup-note-protect-subject`.
- [SAM 3 click select](sam3-click-select.md): Object Selection and Quick Selection select with SAM 3 (click, ⇧/⌥ refine, hover), with a Classic fallback and an engine preference. Branch `feat/sam3-click-select`.
- [SAM 3 box and Select Subject](sam3-box-subject.md): Object Selection's dragged rectangle is SAM 3's box prompt, `sam3` joins the Select Subject engine preference, Swedish AI Models text. Branch `feat/sam3-box-subject`.
- [Shortcuts window](shortcuts-window.md): Keyboard Shortcuts and Menus is resizable with one height for its tabs, records shortcuts like VS Code (chords are two keys in a row), has a reset icon, right-click Remove, modifier-word search, and colour dots on the Menus tab. Branch `feat/shortcuts-window`.
- [Settings sync](settings-sync.md): Export Settings… / Import Settings… move every setting to another Mac in one `.pcsettings` file; the brush size in the options bar is never cut. Branch `feat/settings-sync`.
- [Levels and Curves clipping](levels-curves-clipping.md): ⌥-drag the black or white slider in Levels, or the black or white point in Curves, to see which pixels clip; Curves gets point sliders and Show Clipping (upstream PR #3693 taken early). Branch `feat/levels-curves-clipping`.
