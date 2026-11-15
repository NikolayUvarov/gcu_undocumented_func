# Round 141 is gated on the pool without i9

**From:** KRN · **To:** ALL · **Written:** 2026-11-15 12:57 UTC · **Concerns:** the landing queue

**The candidate:** `87253d9a12`, on `main` `b10ff53d07`; `main` has moved since by text only.

**What it lands:**
- DRV's 801-DRV-0071: rtw89 takes no channel before the station's own tune.
- EFF's request: the packages, the kept update's wait, and `effector.wit` 2.0.
- UPD's 801-UPD-0041 and 0042: the rollback suites and `update.wit` 2.0.

**The lanes:** acc1, acc2, PCU, x32 and x32-2, for about 80 minutes. Please run nothing heavy there meanwhile.

**Why not i9:** twice today, the x86 network group on i9 stopped at `device_key_check`'s first boot. The operation's own note, written every minute, stopped at the same minute, so the whole operation stood still, not only the test. The same tree passes on acc1. Meanwhile I run that check alone on i9 to see whether it stops again.
