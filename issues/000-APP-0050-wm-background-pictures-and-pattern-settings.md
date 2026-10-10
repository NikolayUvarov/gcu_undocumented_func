# 000-APP-0050 — `wm`'s background: PNG and JPEG pictures, the pattern's kind, speed, contrast and complexity, a paler clock

**Type:** tools (`wm`, `libmind`) · **Owner:** tools track (`APP`) · **Priority:** P2 (the maintainer's request, 2026-10-10) · **Status:** in progress · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-3.11, MC-11.5

The maintainer's requests (2026-10-10), after [000-APP-0047](../issues-done/000-APP-0047-wm-desktop-background.done) and [000-APP-0048](000-APP-0048-wm-settings.md):

- "Add PNG and JPEG for the background picture."
- "Add a setting for how fast the background changes (it is very slow now: right for work, but a demo needs it faster) and contrast settings."
- "The clock on the background should be paler."
- "Also settings for the pattern's complexity and its kind, so that there are other abstract variants."

## Problem

The background's picture is a BMP only. Its one pattern moves at one pace with one contrast and one level of detail. The time, the date and the CPU load are drawn brighter than the maintainer wants.

## Plan

- **Decoders in `libmind`**, with no system calls, host-tested:
  - `mind::inflate`: DEFLATE and zlib as a stream over several pieces (a PNG's IDAT chunks are read where they lie), with a 32 KiB window;
  - `mind::png`: every colour type and bit depth, not interlaced, row by row; alpha over a given colour;
  - `mind::jpegdec`: baseline JPEG, greyscale or YCbCr, any sampling, restart markers, a row of blocks at a time; progressive and lossless JPEGs refused by name.
- **`wm`'s cover:**
  - the picture's rows taken in order, so a photo is never held decoded;
  - shrinking averages the pixels each frame pixel covers, growing repeats them;
  - the format told by the file's first bytes; up to 8192 pixels a side and 32 MiB; `wm` asks for 64 MiB.
- **New keys in `data/wm.conf`** (also several on a line between `;`):
  - `pattern = waves | rings | aurora | blobs`;
  - `speed = 1..50`: 1 is today's pace; the pattern moves `speed` times as fast, drawn every 500 / speed ms, at least every 50 ms;
  - `contrast = 0..100`: 20 is today's colours;
  - `complexity = 1..5`: 2 is today's waves;
  - `info = 0..100`: the brightness of what is drawn over the picture; the default, 30, is paler than before.
- **Settings' Background page:** a row for each new key; a change that keeps the picture does not read it again.
- **Docs:** `docs/tools` (EN, RU).

## Acceptance criteria

- **Host tests:**
  - tests/image_host.rs, on pictures `tests/data/images/make.py` writes with Pillow:
    - PNG decoded exactly, IDAT split anywhere included;
    - JPEG against libjpeg's own decoding of each file: within IDCT rounding at full chroma resolution, within a few levels at 4:2:0;
    - damaged and cut files refused without a panic.
  - tests/wm_host.rs:
    - the keys parsed and written back;
    - each pattern within its contrast's colours, moving, different from the others, busier with complexity;
    - the speed's pace;
    - PNG and JPEG covers, the shrinking average among them;
    - the paler information;
    - the Settings rows.
- **The `wm` suite (x86):**
  - a BMP, a PNG and a JPEG of two halves cover the desktop with their colours;
  - `pattern = aurora; speed = 20; …` on one line is read, and the pattern moves far in half a second;
  - the default's text is drawn in the paler colour.

## Related

- [000-APP-0047](../issues-done/000-APP-0047-wm-desktop-background.done): the background.
- [000-APP-0048](000-APP-0048-wm-settings.md): Settings.
- [000-APP-0049](000-APP-0049-network-traffic-on-wms-background.md): the network traffic.
