# Local ML models

PhotoCraft's optional ML features run local ONNX models through the `photocraft-ml` crate and `tract-onnx` 0.23.9 (MIT OR Apache-2.0). Models are downloaded into a user cache directory; they are not committed, bundled, or used by the WebAssembly build.

## Commands

- `select.subject`, `layer.removeBackground` and `edit.removeSubject` accept `engine: auto|macos|model|classic` (`classical` is accepted as `classic`); without it the preference `tools.selectSubjectEngine` decides. `auto` uses the Mac's own cutout on a Mac, then BiRefNet-lite when installed, then the classical implementation. `macos` uses the cutout and falls back to classic. `model` uses BiRefNet-lite; without the model installed (or on a computer with less than 12 GB of memory) it uses classic and says so in the status bar. `classic` avoids model inference.
- `edit.removeSubject` accepts `fill: auto|lama|patch`. `auto` uses LaMa when installed and otherwise uses the patch based fill; `patch` avoids model inference; `lama` requires the model.

## Install

In the app, **Edit › Preferences › AI Models** lists the models with their size and licence, and downloads (with progress and cancel) or removes each one; nothing is fetched until you click Download, and no image ever leaves the computer. On the command line, `photocraft-cli models fetch` downloads the models into the resolved models directory (`photocraft-cli models list` shows what is installed). Both verify the size and SHA-256 before installing. To install manually, put the files in the model's folder in the shared models folder (below), or in a folder named by `PHOTOCRAFT_MODELS_DIR`:

```sh
models="$HOME/Library/Application Support/ArtCraft/models"
mkdir -p "$models/birefnet-lite" "$models/lama"
curl -L --fail -o "$models/birefnet-lite/birefnet_lite.onnx" https://huggingface.co/onnx-community/BiRefNet_lite-ONNX/resolve/main/onnx/model.onnx
curl -L --fail -o "$models/lama/lama_fp32.onnx" https://huggingface.co/Carve/LaMa-ONNX/resolve/main/lama_fp32.onnx
```

A model is looked for in this order, and the first folder that holds all of its files (each with the published size and SHA-256) wins:

1. `PHOTOCRAFT_MODELS_DIR`, when set (then no other folder is used): `<dir>/<model-id>/`, else the files directly in `<dir>`.
2. The folder shared by the Craft apps, so a model is never downloaded twice: macOS `~/Library/Application Support/ArtCraft/models/<model-id>/`, Linux `$XDG_DATA_HOME/artcraft/models/<model-id>/` (or `~/.local/share/artcraft/models/...`), Windows `%APPDATA%\ArtCraft\models\<model-id>\`.
3. The old folder, read only, with the files directly in it: macOS `~/Library/Application Support/PhotoCraft/models`, Linux `$XDG_DATA_HOME/photocraft/models`, Windows `%APPDATA%\PhotoCraft\models`.

New downloads go into the first folder of step 1 or 2. Nothing is ever written to or deleted from the old folder. Preferences › AI Models and `photocraft-cli models list` say which folder each model was found in. The model ids are `birefnet-lite` and `lama`. A model may have several files, each with its own pinned size and SHA-256; it counts as installed only when all of them are there.

| File (in the model folder) | Model and input | Published size | SHA-256 | License | Source file |
|---|---|---:|---|---|---|
| `birefnet-lite/birefnet_lite.onnx` | BiRefNet-lite, 1024 × 1024 | 224,005,088 bytes | `5600024376f572a557870a5eb0afb1e5961636bef4e1e22132025467d0f03333` | MIT | [onnx-community/BiRefNet_lite-ONNX](https://huggingface.co/onnx-community/BiRefNet_lite-ONNX/blob/main/onnx/model.onnx) |
| `lama/lama_fp32.onnx` | LaMa, 512 × 512 | 208,044,816 bytes | `1faef5301d78db7dda502fe59966957ec4b79dd64e16f03ed96913c7a4eb68d6` | Apache-2.0 | [Carve/LaMa-ONNX](https://huggingface.co/Carve/LaMa-ONNX/blob/main/lama_fp32.onnx) |

## Runtime and licences

Inference runs on the CPU. A 1024-pixel inference can take tens of seconds; performance is being measured. Classical selection and patch-based filling remain available without model files.

The model weights keep their upstream licences: BiRefNet-lite is MIT and LaMa is Apache-2.0. The runtime dependency, `tract-onnx` 0.23.9, is MIT OR Apache-2.0. Model files are not included in PhotoCraft distributions; see [`ATTRIBUTION.md`](../ATTRIBUTION.md).
