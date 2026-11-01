# Open as Layers interface (for LightCraft)

LightCraft's **Edit In ▸ Open as Layers in PhotoCraft** hands several photos to PhotoCraft, which opens them as
the layers of one new document (double and multiple exposures). Agreed 2026-10-28; version 1.

1. LightCraft writes the photos as files (as for Edit in PhotoCraft), then a small UTF-8 JSON file with the
   extension **`.pcstack`** next to them:

   ```json
   {
     "kind": "photocraft-open-as-layers",
     "version": 1,
     "name": "DSC01234 + 2",
     "files": ["/abs/path/top.psd", "/abs/path/middle.tif", "/abs/path/bottom.jpg"]
   }
   ```

   - `files`: absolute paths, **top layer first**, 1 to 256 of them, in any format PhotoCraft opens.
   - `name`: optional; the new document's name (default: the first file's name without its extension).
2. LightCraft opens the `.pcstack` file the way it opens a file today: `open -a <PhotoCraft.app> <file.pcstack>` on
   macOS, `photocraft <file.pcstack>` on Windows and Linux. This works whether PhotoCraft is running or not.
3. PhotoCraft makes one new, unsaved document:
   - canvas: the largest width × the largest height of the files; each smaller image centred on it;
   - one layer per file, named after the file without its extension, in the order given (first file on top);
   - colour mode, bit depth, resolution and profile of the top file (the others are converted to it).
4. A file that is missing or can't be read is skipped and named in a notice; no file left gives an error notice
   and no document. A malformed `.pcstack` (wrong `kind`, newer `version`, bad JSON, no files) gives an error
   notice. Nothing crashes.
5. PhotoCraft never deletes or changes the `.pcstack` file or the photos; the folder belongs to LightCraft.
6. The same action is the command `file.openAsLayers` (`{"paths": [...], "name": "..."}`), so the control
   channel, CLI and MCP can use it too.
7. LightCraft can tell whether the installed PhotoCraft has this: its `Info.plist` lists `pcstack` among
   `CFBundleDocumentTypes`. When it doesn't (the official PhotoCraft), LightCraft opens the files one by one,
   as today.
