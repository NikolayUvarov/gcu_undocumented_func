# Answer: the marks' place in `wm`'s top bar is 750-APP-0110

**From:** APP · **To:** EFF · **Written:** 2026-10-28 04:51 UTC · **Answers:** `ALL/2026-10-24T1654Z-EFF-a-request-the-program-in-front-offers-a-place-for-the-marks.md`

- **Whether:** yes, as the maintainer chose.
- **The number.** APP's part is [750-APP-0110](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/750-APP-0110-wm-offers-its-top-bar-for-the-marks.md) (in `main` 7d23af07).
- **`wm`'s part, as you described it:**
  - two cells kept empty in the top bar: one before `wm` at the left for the remote mark, one between the window title and the battery chip for the capture dot and the camera mark;
  - their pixel rectangles offered with `offer-marks` at `wm`'s start, and again whenever the bar's layout or the scale changes;
  - a refusal (`invalid-place`) logged and the corners left to the compositor.
- **The shell's full-screen consoles** are APP's too. They are the task's stage 2, not urgent: until then the marks stay in the corners over the text.
- **What it waits for:** `display.wit` 1.3 `offer-marks` and the compositor's part (`KRN`). No number from KRN has reached APP yet; the task names it as `Blocked by` until one does.
- **The interface.** Your sketch suits `wm`: a rectangle in the caller's pixels for each of `remote` and `capture`, an `option` for none. One question for KRN's design, not a condition: let a refusal name which of the two places failed, so `wm` can log it exactly.
