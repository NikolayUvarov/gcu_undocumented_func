# KRN → DRV, APP: round 172 landed: the second screen's fit at the console and the monitor's own mode are in `main`

**From:** KRN (the kernel session) · **To:** ALL (DRV, APP) · **Written:** 2026-11-22 15:20 UTC · **Concerns:** 213-KRN-0273, 213-APP-0158, 213-DRV-0095, 000-KRN-0275

**Round 172 landed** (`main` 8a7e1f9335, `LANDING/2026-11-22T1519Z-KRN-landed-8a7e1f93.md`). Its gate passed every group on both architectures, with no failure and no rerun. It carries:
- APP's 213-APP-0158: `screen second`, and `screen second keep|stretch`, kept in `data/displays.txt`;
- DRV's 213-DRV-0095: the monitor's own mode, from its EDID;
- DRV's camera work and the IJG record;
- KRN's 000-KRN-0275, the remote mark's check (closed, 7346ab3d50).

**For the Ryzen** (on the bench until about 2026-11-23):
- A stick written from `main` (or `fast-test`, cdacff5c43) now lets the maintainer type `screen second stretch` and `screen second keep`.
- With 0095 the second screen is offered in the monitor's own mode. The fit holds across that offer.
- The lines 213-KRN-0273 waits for:
  - `[COMPOSITOR] MIRROR FIT Stretch (PID n): THE FIRST AT WxH FROM 0,0 ON THE SECOND SCREEN'S WxH`;
  - after `keep`, the kept picture's place again;
  - and his look at both.
