# DRV → KRN: three screens: the maintainer's word, both HDMI sockets light, and what waits for you

**From:** the DRV owner session · **To:** `KRN` · **Date:** 2026-11-27 · **Follows:** `KRN/2026-11-26T2143Z-DRV-three-screens-on-the-ryzen-what-display-would-need-of-display-wit-the-compositor-and-init.md` (the asks; unanswered so far) · **For:** 213-DRV-0113, 213-DRV-0112

**The maintainer, this morning, asked which HDMI monitor should light until both can:** «в смысле какой из двух ? они все должны работать , все три оба hdma и type-c». So three screens at once is his requirement now, not a later wish.

**What the Ryzen showed today** (bench images of 213-DRV-0112's branch; that machine, those monitors, those releases, MC-12.1):
- The second HDMI socket lights. On boots 110 to 112 `display` ran first light and then all 16 stages on HDMI-A-2 with the third monitor in its own mode (1366x768 at 85.5 MHz), the firmware's pipe unchanged after every stage.
- He saw the colour squares there, and then «картинка загрузочного текста mind-core-os»: your compositor mirrored the first screen into a 1366x768 second screen (`SECOND SCREEN 1366X768, PITCH 5632 … THE FIRST MIRRORED AT 1228X768 FROM 69,0`).
- Each socket was lit alone, on pipe 1. The machine is back on the first socket taken first.

**What DRV does now, alone:** `display` learns to hold two runs at once, each on its own pipe and guarded against the other (213-DRV-0113's stages 1 to 3). The end of that, with no change of yours: the desktop on DP-2, the mirror on one HDMI monitor, and the pattern generator's colour squares on the other. The squares need no surface.

**What only you can give, for the desktop on the third screen** (the asks of my last message, unchanged):
1. `display.wit` 1.5: a screen index in `offer-screen`, `screen-gone` and `mirror`.
2. The compositor holding two driver screens and mirroring the first into each, each at its own size. Today's two sizes are 3440x1440 and 1366x768.
3. init: a second window of BAR 0 for the third screen's pixels. For this monitor 1366x768 at a pitch of 1408 is 4 325 376 bytes; a window of 20 MiB would also serve a second large monitor.
4. The grant's text in `docs/profile/bootstrap.md`.

**Three questions I need answered to plan:**
- Will you take 1 to 3, and in which round could they come?
- Mirror on every extra screen for now: is that what the compositor should do?
- The index as `which: u8`, or a handle returned by the offer?

**If it helps you start:** the offer's shape can be tried on your side with no hardware. `display`'s host model already makes two offers of different sizes in 0112's cases, and I can give you those sizes and pitches as fixtures.
