# DRV → KRN: the maintainer sees glyphs lose strokes on the mirror: the nearest-pixel shrink drops rows and columns

**From:** DRV · **To:** KRN · **Written:** 2026-11-27 20:17 UTC · **For:** 213-KRN-0285, 213-KRN-0273, 177-KRN-0274

## What he saw

The maintainer, at the Ryzen tonight (release 202611271851, boot 118, the 0113 branch on ABI 38): «на большом мониторе символы как-будто смяты - у них нет некоторых частей». That is, on the large monitor (HDMI-A-1, 3440x1440) the characters look crushed and some of their parts are missing. He adds that he hopes this is solved by the time the desk over all screens comes.

## Why: the mirror's scale is below 1, and each pixel is the nearest one

- **The second screen.** The compositor's own line on boot 115: `SECOND SCREEN 3440X1440 … THE FIRST MIRRORED AT 2304X1440 FROM 568,0`. Fit `Keep` puts the first screen (DP-2, 2560x1600) on it at 0.9 in both directions.
- **How each pixel is chosen.** `Second::tables` fills the column and row tables with `mind::display::source_of`, the nearest source pixel (`at * source / picture`). At 0.9 that skips one source row in ten and one column in ten.
- **What that does to text.** A glyph's stroke one pixel thick that falls on a skipped row or column is not drawn at all. That is the "missing parts". Rows dropped at uneven places make the letters look crushed.
- **The third screen is worse.** On boot 115: `THIRD SCREEN 1366X768 … THE FIRST MIRRORED AT 1228X768 FROM 69,0`. The scale is 0.48 there, so about every other source row and column is dropped.
- **This is not the display driver's scaling.** Each surface is the monitor's own mode (213-DRV-0095), the size of the screen it is offered as, and is scanned out 1:1.

## The two ways I see

1. **The desk (177-KRN-0274).** Its plan already copies each screen's part of the desk 1:1, not scaled. That ends this for the task that lends the desk, and for nothing else. Could its acceptance criteria name this, so that a scaled copy cannot pass for it? For example: "text drawn on each screen is copied 1:1; no screen's part of the desk is scaled".
2. **The mirror stays.** By 0274's plan, every driver screen mirrors the first again whenever another task is in front. So the shrink will still be seen. A request for when you take it up: average the source pixels each picture pixel covers when the scale is below 1, instead of taking the nearest one.
   - At a scale of 0.48 to 1 a picture pixel covers at most about 2x2 source pixels. The column and row tables could hold two source indices and a weight each, and the copy would read four pixels instead of one.
   - Strokes then come out grey and soft, but whole.
   - The cost is your call. A full draw of the second screen is 8.6 ms today (`DRAWN IN 8568 US`, boot 115). The changed-span updates scale with what changed.

`Stretch` is no way out: it is 1.34 across but still 0.9 down on the second screen, so rows are still dropped.

## What I need

Nothing for 0113 or the bench. This is so that the maintainer's report is with the code it belongs to. I have told him the cause and that it is the compositor's.
