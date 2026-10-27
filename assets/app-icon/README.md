# DarkRaw app icon

A **camera aperture with six blades** in a cyan-to-magenta gradient, inside a glowing ring, on a
dark navy rounded-square tile.

## Geometry

The source is a 512 px picture of the tile (`darkraw-source.png`). The tile is
cut out and its corners made transparent (corner radius about 19 % of the side, upscaled with Lanczos for the large sizes). The macOS renders
pad it onto Apple's 824/1024 icon grid; the Windows and Linux renders use the tile edge to edge.

## Provenance

Generated with ChatGPT (OpenAI) from a prompt by the DarkRaw project owner, who supplied it as
`darkraw-source.png`. Purely AI-generated images may not be protected by copyright in some
countries. License: see `LICENSE.txt`.

## Files

- `darkraw-source.png`: the source picture every other file is rendered from.
- `darkraw-1024.png`: 1024 px render on the macOS grid.
- `darkraw.icns`: macOS bundle icon (`CFBundleIconFile`).
- `darkraw.ico`: Windows icon, 16–256 px, embedded in the `.exe` by `apps/photocraft/build.rs`.
- `hicolor/<size>/apps/ai.storyteller.photocraft.png` (16–512) and `hicolor/scalable/...svg`
  (the 512 px render wrapped in an SVG): Linux icon theme.
- `photocraft.svg`, `photocraft-small.svg`: the previous PhotoCraft artwork, kept only because a test
  in `crates/io/src/svg.rs` opens `photocraft.svg` as a sample file and the macOS DMG background
  links `photocraft-small.svg`. They are not used for the app icon any more.

The file names keep the `photocraft` prefix so `build.rs`, the packaging scripts and the tests keep
finding them.

The app also sets the window icon and Wayland app ID at runtime (`apps/photocraft/src/app_icon.rs`,
`main.rs`). On Windows the taskbar shows the icon of the Start Menu shortcut that launches the
exe, so the MSI's `<Icon Id>` keeps the `.exe` extension (ICE50); `app_icon.rs` documents the
details and tests the `.ico` sizes and the WiX icon references.

## Regenerate

Replace `darkraw-source.png`, adjust `TILE_BOX` in `packaging/icons.py` if the tile sits elsewhere,
then run `packaging/icons.sh` (or `python packaging/icons.py`). It needs only Pillow and writes the
`.ico` and `.icns` itself.
