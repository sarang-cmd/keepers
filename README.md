# Keepers

**Sort a photo dump into collections, then build the final cut.**

Keepers is a single self-contained HTML file. Drop your trip's photos and videos into it, swipe or click through the dump, and sort everything into collections until only the keepers are left. Nothing is uploaded anywhere. Files are read straight from disk in the browser.

- Formerly called *Dump Curator*. Sort maps exported by the old name still import.
- One file, no build step, no dependencies to install, no server.

---

## What it does

- **Swipe view**, keyboard or touch, with a card that fits the photo instead of the other way round: 4:3 stays 4:3, panoramas get a wide card, portraits get a tall one, and video sizes itself from the clip's own metadata. `←` reject, `→` candidate, `Space` later, `↑` final cut, `↓` skip, `Z` undo. Drag the card if you prefer gestures.
- **Compare two photos**, side by side, from anywhere: pick two cells in the grid and hit Compare, press `C` while swiping, or use Compare two at a time inside Final cut to walk the collection as pairs. Each side has Keep, Later and Reject, so a near duplicate sits one click from its twin. Keep and Reject pull the next photo into that slot, so you keep judging without reopening anything. `1` and `2` keep the left or right photo, `Q` and `W` reject, `S` swaps the sides, `N` jumps to the next pair.
- **Quick reject in the grid**: a small red cross on every tile. Hover a photo, click the cross, it leaves the collection with an undo in the toast.
- **Guide lines** over the photo in swipe and compare: rule of thirds, golden ratio, centre cross, diagonals or a fine grid, with colour, width and opacity in Settings.
- **Grid view**, virtualized: only the rows on screen live in the DOM, so a folder of a thousand photos scrolls at the same cost as a folder of thirty. Click to select a bunch, then move them all at once. Undo from the toast.
- **Collections**: Inbox, Candidates, Save for later, Final cut, Rejected, plus your own. Counts and progress stay visible.
- **Final cut**, reorder slides, cap is 20 (Instagram carousel), copy filenames in order, get music ideas, keep notes/caption.
- **Search and filters**, filter by photos / videos / HEIC only, and search by filename.
- **Settings**: theme (auto/light/dark), thumbnail size, guide lines, full-screen preview size, cached tile quality, parallel decoding, cache controls (usage in MB, clear, pre-cache the decoder).

## What you can take out of it

Two downloads, both from Settings:

| Download | What you get |
|---|---|
| **Sort map** (`.json`) | Every decision, your collections, final order, notes and settings. Restore it later and the same folder sorts itself instantly. Files match by name, size and modified time, with a filename fallback. |
| **Sorted ZIP** | One folder per collection (`Candidates/…`, `Final cut/01 - cover.heic`) plus a `manifest.json`. Original files, not previews. |

The toolbar `↓` button zips just the collection you are looking at.

## HEIC / HEIF

Browsers can't decode HEIC except Safari, so Keepers uses [libheif](https://github.com/strukturag/libheif) compiled to WASM (via [`heic-to`](https://github.com/hoppergee/heic-to)).

- The decoder (~3 MB) is downloaded **once**, then stored in IndexedDB and served from there, so HEIC keeps working offline.
- After a folder opens, Keepers pre-caches every photo in the background: one small tile per HEIC, built inside the worker. A progress row in the sidebar shows the count and carries a stop button.
- Decoding runs in a **Web Worker**, so libheif never blocks the interface. The worker hands back a ready `ImageBitmap` and the small cached tile in one message. Browsers without `OffscreenCanvas` fall back to decoding on the main thread automatically.
- **Viewing never converts anything.** libheif hands back pixels and they are painted on a `<canvas>` at exactly the size needed. No JPEG round trip, no intermediate file, no full size copy in memory for 48 MP shots.
- The only encoded artifact is a small cached tile (≤ 480 px, roughly 30 KB per photo) so a grid of hundreds of HEICs reopens instantly and works with no network at all.
- Large files are decoded straight to display size (`ispe` box is read first), which keeps a 48 MP photo from blowing up a tab's memory.

MOV/MP4 playback is left to the browser, so HEVC video still needs Safari.

## Privacy

Everything happens locally: no uploads, no analytics, no accounts. The only network requests are the font (optional, loads without blocking) and the HEIC decoder on first use. Sort decisions and cached tiles live in `localStorage` and IndexedDB of the browser you opened the file in.

## Usage

1. Open `index.html` in Chrome, Edge, Firefox or Safari (double-clicking the file is fine).
2. **Choose folder** (or choose files).
3. Sort. Press `,` for settings at any time.
4. Download the sort map and/or the sorted ZIP when you're done.

Sorting state persists per browser, so you can close the tab and come back.

## Keyboard

| Key | Action |
|---|---|
| `←` / `→` | Reject / Candidate |
| `Space` | Save for later |
| `↑` / `↓` | Final cut / Skip |
| `1`–`9` | Move to collection N |
| `Z` | Undo |
| `C` | Compare this photo with the next one |
| `/` | Focus search |
| `,` | Settings |
| `Esc` | Close menu / clear search |

## Notes on the source

Single HTML document: inline CSS, inline JS, inline SVG data-URI favicon. The ZIP writer is a ~60 line store-only implementation (CRC32, local headers, central directory) so there is nothing to load at runtime. Swipe actions patch counters in place instead of rebuilding the shell. Compare is a third view beside swipe and grid: it mounts two media boxes, sizes them from the wider of the pair, and repaints one side at a time when a judgement lands. Guide lines are an SVG overlay with non-scaling strokes, so the width you pick looks the same at any photo size. The grid keeps a translated window of cells around the viewport and recycles them on scroll, and HEIC work is posted to a worker that owns its own copy of the decoder.

## License

MIT
