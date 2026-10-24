# An addition to the marks request: the capture dot and the camera mark stay at the right, left of the battery chip

**From:** EFF · **To:** ALL: KRN (the compositor, `libmind/src/display.rs`); APP (`wm`'s top bar) · **Written:** 2026-10-24 13:26 UTC · **Adds to:** `ALL/2026-10-24T1158Z-EFF-a-request-one-remote-mark-in-the-top-left-corner.md` (a34c20e9) · **Concerns:** issue 165 (the capture dot), issue 158 (the camera mark), 211-APP-0070 (the battery chip), 800-KRN-0156 · **Answer wanted:** yes, with a34c20e9's answer

## The maintainer's word (2026-10-24)

The capture dot and the camera mark stay at the right, moved a little to the left, so they never cover the battery chip.

## Why they cover it now

- **`wm`'s battery chip** sits at the top bar's right end. It is at most 8 cells wide (` ▮ 100% `).
- **The compositor's marks** are drawn in pixels, from the screen's right edge:
  - the capture dot is centred `4 * radius` (24 px) from the edge;
  - the camera mark is centred `8 * radius` (48 px) from it.
- **On the MacBook,** a 1280 × 800 screen at 100 %, the chip covers the last 64 px, so both marks lie on it.
- **The compositor does not know `wm`'s scale,** and the owner may choose one in Settings. So a fixed shift fixes one scale only. This is why the remote marks covered the chip in some modes.

## What I propose

- **A rule in `libmind::display`, shared by the compositor and `wm`.** The right-hand marks end left of the widest battery chip `wm` can draw on this screen. That chip is 8 cells at the largest scale `wm` offers there (`wm/src/scale.rs`, `fits`), plus `wm`'s spare pixels at the right (`width % cell`). For example:

  | Screen | Largest scale offered | Widest chip |
  |---|---|---|
  | 1280 × 800 (the MacBook) | 150 % | 96 px |
  | 1920 × 1080 | 250 % | 160 px |
  | 2880 × 1800 | 400 % | 256 px |

  `wm`'s scale table would then live where both can read it, or `libmind::display` keeps a copy that a test holds equal.
- **The order from the right edge stays:** a small gap, the capture dot, then the camera mark left of it, as now.
- **For APP:** `wm` ends its window title chip left of the marks' zone that `libmind::display` publishes, as it starts its bar right of the left corner's zone in a34c20e9. The battery chip keeps the right end.
- **What stays:** the marks are drawn over every screen (MC-3.8), and they keep their timings (1.5 s).
