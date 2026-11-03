# Keepers

**Sort a photo dump into collections, then build the final cut.**

Keepers is a single self-contained HTML file. Drop your trip's photos and videos into it, swipe or click through the dump, and sort everything into collections until only the keepers are left. Nothing is uploaded anywhere. Files are read straight from disk in the browser.

- Formerly called *Dump Curator*. Sort maps exported by the old name still import.
- One file, no build step, no dependencies to install, no server.

---

## What it does

- **Swipe view**, keyboard or touch: `←` reject, `→` candidate, `Space` later, `↑` final cut, `↓` skip, `Z` undo. Drag the card if you prefer gestures.
- **Grid view**, lazy-loaded tiles, click to select a bunch, then move them all at once. Undo from the toast.
- **Collections**: Inbox, Candidates, Save for later, Final cut, Rejected, plus your own. Counts and progress stay visible.
- **Final cut**, reorder slides, cap is 20 (Instagram carousel), copy filenames in order, get music ideas, keep notes/caption.
- **Search and filters**, filter by photos / videos / HEIC only, and search by filename.
- **Settings**: theme (auto/light/dark), thumbnail size, full-screen preview size, cached tile quality, parallel decoding, cache controls (usage in MB, clear, pre-cache the decoder).

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
- After a folder opens, Keepers pre-caches every photo in the background: one small tile per HEIC. A progress row in the sidebar shows the count and carries a stop button.
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
| `/` | Focus search |
| `,` | Settings |
| `Esc` | Close menu / clear search |

## Notes on the source

Single HTML document: inline CSS, inline JS, inline SVG data-URI favicon. The ZIP writer is a ~60 line store-only implementation (CRC32, local headers, central directory) so there is nothing to load at runtime. Swipe actions patch counters in place instead of rebuilding the shell.

## License

MIT
