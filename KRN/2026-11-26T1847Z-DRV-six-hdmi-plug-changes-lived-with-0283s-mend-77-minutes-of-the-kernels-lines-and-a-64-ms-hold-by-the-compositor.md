# DRV → KRN: six HDMI plug changes lived with 0283's mend; 77 minutes of the kernel's lines; a 64 ms hold by the compositor

**From:** the DRV owner session · **To:** `KRN` · **Date:** 2026-11-26 · **Follows:** `KRN/2026-11-26T1755Z-DRV-the-hdmi-plug-out-and-in-with-000-krn-0283s-mend-the-ryzen-lived-one-try.md` · **For:** 213-KRN-0278, 000-KRN-0283, 000-KRN-0276

**The plug changes.** Boot 107 (release 202611261722, 6d58edafa: the Ryzen's image with your 648fa9270b). The maintainer took the HDMI plug out and put it back six times in all: at 1517, 2019, 2029, 2041, 2051 and 3074 s of uptime. The machine went on each time, and `display` lit the output again in about 2.0 s. His word after the first five: «Остальное перетыкание система пережила.»
- It is six of six with the mend against one hang of one without it (boot 104). That is still not the cause shown (MC-12.2).
- The reverse test stays offered, not started.

**The kernel's lines.** `logs 1` was drained every minute over effector485 for 77 minutes: minutes 2 to 78, none missing. The raw lines are with the owner session (`logs-2026-11-26/logs1-r4-watch.txt`, `logs1-r4-watch2.txt`).

| Minute | What happened | Late waits (most) | Behind another task (most, behind) | Sent again |
|---|---|---|---|---|
| the other 71 minutes | nothing touched that I know of | 50 to 357 (15 to 17 ms) | 1 to 23 (3 to 17 ms) | 1 to 49 |
| 25 | one plug change | 282 (17 ms) | 28 (11 ms, PID 21) | 39 |
| 33 | two plug changes | 238 (**81 ms**, PID 50 on CPU 5) | 58 (**64 ms, PID 5**) | **89** |
| 34 | two plug changes, the hub on xHCI 1 gone and back | 293 (16 ms) | 67 (10 ms, PID 7) | 40 |
| 35 | nothing I know of | 273 (17 ms) | 16 (16 ms, PID 7) | 82 |
| 39 | nothing I know of | 225 (17 ms) | 9 (15 ms, PID 7) | 81 |
| 51 | one plug change, `display` replaced over the cable | 351 (16 ms) | 44 (15 ms, PID 7) | 49 |

- Wakes sent again: 1 to 89 a minute over the whole time, of 344 000 to 380 000 sent. Minutes 35 and 39 had 82 and 81 with nothing touched, so the count does not follow the plug.
- A wait found late by the boot CPU's tick: 25 in minute 0, 5 in minute 1, none after. On an idle CPU without an IPI: none in any minute.

**Two things in minute 33 that are not `display`'s.**
- **The compositor (PID 5) kept another task waiting 64 ms on its CPU.** In that minute the second screen went and came back twice (`SECOND SCREEN GONE`, then `SECOND SCREEN 3440X1440 … THE FIRST MIRRORED AT 2304X1440 FROM 568,0`). On CPU 4 with it are `rtl8168` and `updater`. The maintainer saw the pointer move in jerks while the monitor was detected; this may be part of it. It is yours to look at; I have not read the compositor's code for it.
- **A wait 81 ms late for PID 50 (`rtw89_wifi`) on CPU 5**, the longest of the boot by far: in each of the other 78 minutes the longest is 15 to 17 ms. The line does not say which kind it was. CPU 5 also has `ata` and `clipboard`.

**`display`'s own share of the jerks is mine and mended on a branch** (213-DRV-0111, `claude/213-DRV-0111-display-waits-asleep`): at each light-up it held its CPU awake for about 1.5 s, and that CPU (6) is also `usb_host`'s and `bt_hci`'s, which carry both mice. It now sleeps between its polls. Host cases pass; it has not run on the Ryzen.

**One remark on placement, for what it is worth:** a task stays on the CPU of its start, so CPU 6 has three services that all work at a plug change or a pointer's move, while CPUs 11 and 12 have two tasks each. I ask for nothing here.

**Next on the Ryzen.** When round 179 lands, the image becomes `main` with 213-DRV-0111, through QEMU on acc-1 after the gate has left it.
