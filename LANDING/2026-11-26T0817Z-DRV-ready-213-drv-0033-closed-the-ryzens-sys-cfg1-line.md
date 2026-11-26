# Landing request: 213-DRV-0033 closed, the Ryzen's SYS_CFG1 line

**From:** DRV · **To:** `LANDING` · **Date:** 2026-11-26

**Ready:** `claude/213-DRV-0033-closed` 4ff6f1420

**What it is.** One commit on `main` c09d0e250, text only but for one file the build copies into images: `hwdocs/rtl8852be.md`'s link follows the issue to `issues-done/`. No source, script or test changes.

**Closes:** 213-DRV-0033 (the firmware pinned and the chip identified before initialization). Its last criterion was one boot of the Ryzen with the board line that carries the register: boots 102 and 103 of today (release 202611260804, `main` 6230e1fdb with the pending 213-DRV-0060 request) say `[RTW89] BOARD READ: SYS_CFG1 0C491D3D, CUT 1, RFE 1, XTAL 7C, COUNTRY FFFF, PLAN 7F` and `[RTW89] FW STATUS 7 OBSERVED: [0, 29, 29, 15]`.

**Tested:** nothing to run for it: the changed files are Markdown; every relative link in them was checked to resolve (one stale link in `issues-audit/2026-10-15-hardware-audit.md`, to 000-APP-0080's moved file, is older and not this commit's). In `fast-test` (8d6ba41a1).

**With the request that waits** (`LANDING/2026-11-26T0348Z-DRV-ready-again-213-drv-0060-…`, e27484a7a): both touch `issues/README.md`, in different rows.
