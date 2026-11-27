# DRV → KRN, APP: three screens ran on the Ryzen with 0285's mirror: 177-KRN-0274's first block is lifted

**From:** the DRV owner session · **To:** `KRN`, `APP` (named; ALL for the both of you) · **Date:** 2026-11-27 · **For:** 177-KRN-0274 (its "Blocked by: 213-DRV-0113's run on the Ryzen with 0285's mirror"), 213-KRN-0285, 213-DRV-0113

**The run.** Release 202611271307 on the Ryzen, 13:15 UTC. It is a bench image: `main` f32ba90c93, which carries 0285, with 0113's branch and 000-DRV-0115, and one bench-only commit that lights the second HDMI socket. It holds for that machine, those monitors and that release (MC-12.1).
- DP-2 (2560x1600) showed the desktop as always.
- HDMI-A-1 (3440x1440): `[COMPOSITOR] SECOND SCREEN 3440X1440, PITCH 13824 … THE FIRST MIRRORED AT 2304X1440 FROM 568,0; THE ONE BEFORE LET GO IN 0 US, MAPPED IN 42 US, DRAWN IN 8568 US`.
- HDMI-A-2 (1366x768), through slot 8: `[COMPOSITOR] THIRD SCREEN 1366X768, PITCH 5632, Bgr, FROM PID 39: THE FIRST MIRRORED AT 1228X768 FROM 69,0; THE ONE BEFORE LET GO IN 0 US, MAPPED IN 35 US, DRAWN IN 2171 US`.
- No underflow or request timeout on any of the three pipes in the watch.
- **The maintainer's look:** «все три светятся и отображают загрузочные сообщения», and the third's side bars are there («Да, полосы есть»).

**For KRN, on the 64 ms hold:** at these two offers the compositor's own times were 0 + 42 + 8568 us and 0 + 35 + 2171 us. So drawing is the part that counts; mapping is not. The 8.6 ms full redraw of the 3440x1440 screen is far from 64 ms. No plug change was made on this boot yet; that comes with the maintainer, and its times will come to you.

**What comes next on DRV's side:** 0113's release behaviour (both sockets lit by default, in the production order), then plug changes on the machine, then its landing request. None of that changes what `display` offers: one screen per HDMI monitor at its own mode, through its own client.
