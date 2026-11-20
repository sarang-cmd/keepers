# Keepers desktop

A native Rust build of Keepers. One window, one binary, no browser tab and no WebAssembly.

## Run it

Download `keepers-1.0.0-windows-x64.zip` from the releases page, unpack it anywhere and run
`keepers.exe`. Nothing to install. The binary is 5 MB, links the C runtime statically and keeps
no state outside `%APPDATA%\Keepers` and your tile cache in `%LOCALAPPDATA%\Keepers`.

Windows may show a SmartScreen warning the first time because the build is unsigned.
More info, then Run anyway.

The window is drawn with wgpu, so it goes through Direct3D 12 on Windows. Any machine that
runs Windows 11 will run it, including over Remote Desktop.

HEIC decoding comes from the HEIF Image Extension that ships with Windows, so there is no
decoder to bundle and no 3 MB WASM download on first use. JPEG, PNG, WebP, BMP and TIFF go
through the same Windows Imaging Component path.

## What it does

- Opens a folder, reads every HEIC and JPEG in it, and builds a 480 px tile for each photo
  in parallel across every core.
- Swipe view with a card sized from the photo, rule of thirds, golden ratio, cross and
  diagonal guides.
- Collections: Inbox, Candidates, Save for later, Final cut, Rejected, plus your own.
- Grid view with quick reject on hover and a two photo compare.
- Search, a photos only or videos only filter, and keyboard sorting.
- Every decision is saved as you go and can be exported to `keepers-sortmap.json` next to
  the photos, then restored on another machine.
- Thumbnails are cached on disk, so a second open of the same folder paints from cache.

## Keys

| Key | Action |
|---|---|
| left | Reject |
| right | Candidate |
| space | Save for later |
| up | Final cut |
| down | Skip |
| z | Undo |
| c | Compare with the next photo |
| / | Clear the search |
| , | Settings |

## Build from source

```
rustup target add x86_64-pc-windows-msvc
cargo install cargo-xwin
cargo xwin build --release --target x86_64-pc-windows-msvc
```

The binary lands in `target/x86_64-pc-windows-msvc/release/keepers.exe`. Building on Windows
itself needs nothing more than `cargo build --release`. On Linux or macOS the `cargo xwin`
line cross compiles it, which is how the published binary was made.

The renderer is wgpu, which is why the build needs the `x11` and `wayland` features on Linux
and none on Windows. The other platforms build too: `cargo build --release` on Linux gives a
working app with the `image` crate decoders, minus HEIC, because libheif is not linked in.
macOS would need an ImageIO path, which is not written yet.

## Tests

```
cargo test --release -- --nocapture
```

Three tests run headless. `full_flow` opens a folder, waits for the tile pass, presses keys,
checks the undo stack, restarts the sort map, exports and imports it, opens compare and closes
it. `cache_is_parallel_and_warm_start_is_instant` measures the cold and warm passes.
`screenshots` rasterizes the real interface with a small software renderer and writes the PNGs
used in the README, so the images always match the code.

## Layout

```
src/main.rs    window shell
src/app.rs     state, panels, the three views, keyboard, persistence
src/decode.rs  Windows Imaging Component decode and the on disk tile cache
src/thumbs.rs  worker pool for the tile pass
src/model.rs   folder scan, sort map, settings, colours
src/theme.rs   palette, panels, buttons, drawn icons
src/shot.rs    software rasterizer for the screenshots
```
