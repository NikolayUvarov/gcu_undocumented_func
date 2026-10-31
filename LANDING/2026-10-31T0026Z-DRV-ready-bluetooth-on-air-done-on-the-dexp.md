# Ready: 700-DRV-0046 and 700-DRV-0047 done, with the DEXP's release run (replaces the request of 00:00)

**From:** DRV · **To:** LANDING · **Written:** 2026-10-31 00:26 UTC · **Concerns:** 700-DRV-0046, 700-DRV-0047 (both close) · **Replaces:** `LANDING/2026-10-31T0000Z-DRV-ready-bluetooth-on-air-and-its-events-without-loss.md`, if not yet answered

**Ready:** `claude/ASR-DRV` bb22c3ca2c511466b0bae4766458f526301d664b

## What it adds to the earlier request

**One text commit, bb22c3ca:** both issues closed with the DEXP's run.
- **Release 202610302359** (tree `d83c98c5`: `ff0da96d` with these two and BLT's 700-BLT-0002), over the cable, slot A confirmed:
  - rtw88's free run logged;
  - the Wi-Fi joined by itself and acknowledged every frame.
- **`bt_hci`'s rounds:** 14 and 15 LE and 1 BR/EDR devices, 0 event packets dropped (64 to 404 before 0047).

**The code is unchanged** since 3cc7184f. The earlier tests stand:
- all host tests;
- the x86 and aarch64 builds;
- the QEMU `usb` suite.

**A text conflict to expect:** BLT's request (next) adds a row at the same place of `issues/README.md`'s finished table. Keep both.
