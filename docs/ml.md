# Local ML models

PhotoCraft's optional ML features run local models through the `photocraft-ml` crate: BiRefNet-lite (ONNX) on `tract-onnx` 0.23.9 (MIT OR Apache-2.0) and LaMa (`big-lama.pt`) on candle 0.9.2 through `photocraft-lama` (`crates/lama`, Apache-2.0). Models are downloaded into the models folder the Craft apps share; they are not committed, bundled, or used by the WebAssembly build.

## Commands

- `select.subject`, `layer.removeBackground` and `edit.removeSubject` accept `engine: auto|macos|model|sam3|classic` (`classical` is accepted as `classic`); without it the preference `tools.selectSubjectEngine` decides. `auto` uses the Mac's own cutout on a Mac, then BiRefNet-lite when installed, then the classical implementation. `macos` uses the cutout and falls back to classic. `model` uses BiRefNet-lite; without the model installed (or on a computer with less than 12 GB of memory) it uses classic and says so in the status bar. `classic` avoids model inference. `sam3` asks SAM 3 for the text prompt "person", then "main subject" when nothing scores (on the Object Selection encoding when it is ready for these pixels, else the job encodes the picture and frees it afterwards); "no subject" is its answer, and a SAM 3 that cannot run (not installed, less than 32 GB, failure) uses Classic and says so. `auto` is unchanged: SAM 3 is never part of it (it needs 6 to 20 s and 19 GB where the Mac's cutout takes 0.1 s).
- `edit.removeSubject` accepts `fill: auto|lama|patch`. `auto` uses LaMa (`big-lama.pt`) when installed and otherwise uses the patch based fill; `patch` avoids model inference; `lama` requires the model.

## Install

In the app, **Edit › Preferences › AI Models** lists the models with their size and licence, and downloads (with progress and cancel) or removes each one; nothing is fetched until you click Download, and no image ever leaves the computer. On the command line, `photocraft-cli models fetch` downloads the models into the resolved models directory (`photocraft-cli models list` shows what is installed). Both verify the size and SHA-256 before installing. To install manually, put the files in the model's folder in the shared models folder (below), or in a folder named by `PHOTOCRAFT_MODELS_DIR`:

```sh
models="$HOME/Library/Application Support/ArtCraft/models"
mkdir -p "$models/birefnet-lite" "$models/lama"
curl -L --fail -o "$models/birefnet-lite/birefnet_lite.onnx" https://huggingface.co/onnx-community/BiRefNet_lite-ONNX/resolve/main/onnx/model.onnx
curl -L --fail -o "$models/lama/big-lama.pt" https://huggingface.co/fashn-ai/LaMa/resolve/main/big-lama.pt
```

A model is looked for in this order, and the first folder that holds all of its files (each with the published size and SHA-256) wins:

1. `PHOTOCRAFT_MODELS_DIR`, when set (then no other folder is used): `<dir>/<model-id>/`, else the files directly in `<dir>`.
2. The folder shared by the Craft apps, so a model is never downloaded twice: macOS `~/Library/Application Support/ArtCraft/models/<model-id>/`, Linux `$XDG_DATA_HOME/artcraft/models/<model-id>/` (or `~/.local/share/artcraft/models/...`), Windows `%APPDATA%\ArtCraft\models\<model-id>\`.
3. The old folder, with the files directly in it: macOS `~/Library/Application Support/PhotoCraft/models`, Linux `$XDG_DATA_HOME/photocraft/models`, Windows `%APPDATA%\PhotoCraft\models`. It only matters until the app has moved its files (below).

New downloads go into the first folder of step 1 or 2. **At every start** (the app, and `photocraft-cli models`; not while `PHOTOCRAFT_MODELS_DIR` is set), the app moves each model file from the old folder into the shared one (`migrate.rs`): a rename on the same disk, after the file passes the published size and SHA-256. A verified copy already in the shared folder is never deleted, overwritten or downloaded again: only the old duplicate goes (a shared copy that fails the check is replaced by the verified old one). A move that fails (permissions, another disk) leaves both files, logs why, and the app starts anyway. The old `lama_fp32.onnx` is not used any more: it is removed once `big-lama.pt` is verified in the shared folder. The old empty `.lock` files and the old folder go when empty. Preferences › AI Models and `photocraft-cli models list` say which folder each model was found in. The model ids are `birefnet-lite`, `lama` and `sam3`. A model may have several files, each with its own pinned size and SHA-256; it counts as installed only when all of them are there.

| File (in the model folder) | Model and input | Published size | SHA-256 | License | Source file |
|---|---|---:|---|---|---|
| `birefnet-lite/birefnet_lite.onnx` | BiRefNet-lite, 1024 × 1024 | 224,005,088 bytes | `5600024376f572a557870a5eb0afb1e5961636bef4e1e22132025467d0f03333` | MIT | [onnx-community/BiRefNet_lite-ONNX](https://huggingface.co/onnx-community/BiRefNet_lite-ONNX/blob/main/onnx/model.onnx) |
| `lama/big-lama.pt` | LaMa, 512 × 512 (TorchScript; the file LightCraft uses, in the same folder) | 205,803,670 bytes | `7ba7aa7ac37a4d41fdbbeba3a2af7ead18058552997e3a3cd1a3b2210c9e6b4c` | Apache-2.0 | [fashn-ai/LaMa](https://huggingface.co/fashn-ai/LaMa/blob/main/big-lama.pt) |

## SAM 3 (object selection by click or by text)

`photocraft-sam3` (`crates/sam3`, L4, Apache-2.0 only) runs Meta's SAM 3 in pure Rust on candle 0.9.2: on the GPU through Metal on a Mac, on the CPU elsewhere. It is LightCraft's `crates/segment` (see [`ATTRIBUTION.md`](../ATTRIBUTION.md)). It gives the editor masks from positive and negative clicks and from a text prompt ("the red car"); Object Selection and Quick Selection use its clicks (below); nothing else does yet.

The model id is `sam3`, in `ArtCraft/models/sam3/` (shared with LightCraft). It has three files, each pinned like the others, and it has **no download**: the Hugging Face repository is gated (each user accepts Meta's SAM License there). Put `model.safetensors` (3,439,938,512 bytes, `6d06f0a5…cc14a`), `vocab.json` and `merges.txt` into the folder yourself. Until they are all there, Preferences › AI Models shows where to put them (no Download button), and `photocraft-sam3` returns `MlError::ModelMissing`. PhotoCraft never deletes these files.

The API is `Sam3Service`: `encode(&Rgb)` runs the image through the model once and returns an `ImageHandle`; `clicks(handle, &[Click])`, `boxed(handle, BoxPrompt, &[Click])` (a rectangle, SAM's box prompt: two corner points labelled 2 and 3 in the prompt encoder, with optional extra clicks) and `text(handle, "sky", 0.5)` return a `Matte` (`w × h` coverage, `0..=1`) from that encoding; `unload()` frees the model and `unload_if_idle` / `spawn_idle_reaper` free it after 10 minutes without use. A model failure on the GPU is retried on the CPU; `PHOTOCRAFT_SAM3_CPU=1` keeps it on the CPU.

**Object Selection and Quick Selection by click.** The preference `tools.objectSelectionEngine` is `auto` (SAM 3 when the model is installed and the computer has 32 GB of memory, else Classic), `sam3` (the same, with a status note when it falls back) or `classic`. Picking either tool starts encoding the picture in a background thread (`select.objectPrepare` does the same for scripts: `wait` waits for it, `cancel` stops it; Esc stops it in the app); until it is ready, and when SAM 3 is off, the Classic finders answer. Then a click selects the object under it with SAM 3's soft mask as the selection's alpha, and the hover preview is SAM 3's too. `select.object` with `"refine": "add"` or `"subtract"` (⇧- and ⌥-click) adds a positive or negative point to the mask the last click made; Quick Selection strokes are positive clicks (⌥ strokes negative). `select.object` with a `"rect"` (a dragged rectangle) gives SAM 3 the rectangle as its box prompt and selects the main object inside it, combined with the selection by `mode` (⇧ add, ⌥ subtract, ⇧⌥ intersect); with no encoding ready, or when SAM 3 fails, Classic's rectangle GrabCut answers with the same notes as for clicks. One encoded picture is kept, for the document and layer the tool samples; a pixel edit, closing the document, another document or layer, or ten idle minutes frees it. The 3.4 GB file's SHA-256 is checked once and remembered in `PhotoCraft/model-checks.tsv` (size, modification time, inode, change time and hash), so later starts skip the 6 to 25 seconds the check takes.

Measured on an Apple-silicon Mac with 48 GB (release build, GPU, other builds running at the same time, so the times are the slow side): the model loads in under a second after the files are verified, an image encodes in about 5 s, a click answers in about 50 ms, a text prompt in 0.7 to 2 s. Peak memory: 1.9 GB with the model loaded, 19 GB while an image is encoded, 20.3 GB with a text prompt too, whatever the image's size (the model takes a 1008 px square). Computers with less than 32 GB of memory are refused (`ram.rs`). `crates/sam3/examples/sam3_probe.rs` and the ignored test `crates/sam3/tests/real_model.rs` repeat the measurement.

## LaMa (Remove Subject) on candle

`photocraft-lama` (`crates/lama`, L4, Apache-2.0 only) is LightCraft's pure-Rust LaMa: it reads the TorchScript archive `big-lama.pt` without Python (the pickle is parsed, never run) and runs the FFC-ResNet generator on candle, on the GPU through Metal on a Mac (the CPU when the GPU fails, or with `PHOTOCRAFT_LAMA_CPU=1`). `photocraft_ml::inpaint` keeps PhotoCraft's own window, detail transfer and feathering around it; the network call is `Lama::predict` on the 512 × 512 window. The model is loaded for each run and freed after it. The ONNX LaMa (`lama_fp32.onnx`) is no longer downloaded or read. [`fork/lama.md`](../fork/lama.md) has the measured speed and memory.

## Runtime and licences

BiRefNet-lite runs on the CPU (tract). A 1024-pixel inference can take tens of seconds; performance is being measured. Classical selection and patch-based filling remain available without model files.

The model weights keep their upstream licences: BiRefNet-lite is MIT, LaMa is Apache-2.0 and SAM 3 is under Meta's SAM License. The runtime dependencies, `tract-onnx` 0.23.9 and candle 0.9.2, are MIT OR Apache-2.0. Model files are not included in PhotoCraft distributions; see [`ATTRIBUTION.md`](../ATTRIBUTION.md).
