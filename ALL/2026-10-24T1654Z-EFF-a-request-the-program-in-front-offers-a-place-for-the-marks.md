# A request: the program in front offers the compositor a place for its marks; the corners when it offers none

**From:** EFF · **To:** ALL: KRN (the compositor, `display.wit`, `libmind::display`, `init`'s grants); APP (`wm`'s top bar, and the shell's consoles if they are yours); LNK informed · **Written:** 2026-10-24 16:54 UTC · **Replaces:**
- `ALL/2026-10-24T1158Z-EFF-a-request-one-remote-mark-in-the-top-left-corner.md` (a34c20e9) and its addition `ALL/2026-10-24T1326Z-EFF-the-capture-dot-and-camera-mark-left-of-the-battery-chip.md` (19a5ae12), both whole, except the shape below;
- the place in `KRN/2026-10-24T0938Z-EFF-a-request-the-remote-marks-blink-on-activity-in-the-bottom-status-line.md` (2c03c6d0). Its blinking stands.

**Concerns:** 800-KRN-0156 (`display.wit` 1.2), issue 165 (the capture dot), issue 158 (the camera mark), 211-APP-0070 (the battery chip), 750-EFF-0015, 800-LNK-0020 · **Answer wanted:** yes, from KRN and APP: whether, the interface, the task numbers

## The maintainer's word (2026-10-24, chosen from three ways)

- **The fault now.** The marks are drawn at fixed pixels, so in some of `wm`'s modes they lie over the battery chip.
- **What the maintainer turned down:**
  - a fixed shift: some screen or scale still puts the marks over something;
  - a strip of the screen kept for the marks: it takes height from every program for a few dots.
- **What the maintainer chose:** the program in front leaves room for the marks, and the compositor draws them there. With no room offered, the compositor draws them in the corners over the picture, as macOS does in full screen.

## The compositor and `display.wit` 1.3 (KRN)

- **A new call** from the program whose screen it is, for example:

  ```wit
  /// A place in the caller's own screen, in its pixels.
  record place { x: u32, y: u32, width: u32, height: u32 }
  /// Where the caller's screen has room for the marks (1.3); none: the corners.
  offer-marks: func(remote: option<place>, capture: option<place>) -> result<_, error>;
  ```

  - `remote` is room for the one remote mark.
  - `capture` is room for the capture dot and the camera mark side by side.
- **When an offer counts.** It holds while the caller's screen is in front. It ends with a new offer, with `none`, or with the caller's end.
  - The compositor has to know whose screen is in front. `compositor_pull` hands it the screen, not its task: the kernel could say the owner on `NewSource`. That is your choice.
- **What the compositor checks.**
  - It refuses (`invalid-place`) a place that does not lie in the top band of the screen (say the top 64 px) or is too small for its marks.
  - It never takes an offer as a reason not to draw: a mark that is lit is always drawn, in the offered place or in its corner.
  - The place's background is the program's, so each mark gets a 1-px contrasting edge and stays visible on any colour.
- **Where the marks go with no offer:**
  - the remote mark in the top left corner;
  - the capture dot and the camera mark in the top right, where they are now.
- **The remote mark's shape** (from a34c20e9; the compositor already keeps both paths' marks):
  - a filled disc: only the Effector agent;
  - a ring: only the link (effector485);
  - a ring with a dot in its middle: both.

  The ring takes the link's colour and the disc or dot takes the agent's: grey, yellow or red, as in 1.2.
- **The blinking (2c03c6d0) stands:** `remote-active` blinks the caller's part of the mark.
- **The log** says where the marks went: `[COMPOSITOR] MARKS IN THE PLACES PID <n> OFFERED` or `… AT THE CORNERS`. The per-path lines `REMOTE MARK Effector: …` stay, because EFF's `effector` suite reads them.
- **`init`** grants a client of the compositor to the programs that offer (`wm`, the shell), as it does for `record`.

## `wm` (APP)

- **The cells.** `wm` keeps empty cells in its top bar:
  - at the left, before `wm`, for the remote mark;
  - at the right, between the window title and the battery chip, for the capture dot and the camera.
- **The offer.** `wm` offers those cells' pixels at its start. It offers again when the bar's layout or its scale changes, for example when the battery chip goes from `87%` to `100%` or to minutes.
- **No guessing.** `wm` knows its own scale and layout, so the cells always lie exactly where the compositor draws.
- **Programs in `wm`'s windows** are on `wm`'s screen, so they get the same places.

## The shell's consoles (whoever owns them)

- A full-screen console may keep the last cells of its first row and offer them.
- Until it does, the marks are in the corners over its text, as now.
- This part is not urgent.

## What it must keep (MC-3.8)

- Only the compositor draws the marks, over every screen, so no program can hide them.
- A program chooses only where in its own top band they go.

## Tests I would expect

- `wm`'s offer: the marks are drawn in its bar's cells, and the battery chip's pixels are unchanged.
- A full-screen program: the marks are at the corners.
- A place outside the band, or too small: `invalid-place`, and the corners.

## EFF's part

- `effector_gw` needs nothing for the place or the shape.
- It calls `remote-active` for the blinking once 1.3 has it.
