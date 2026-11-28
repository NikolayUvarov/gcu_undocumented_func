# Landing request: 700-BLT-0011, a device asleep all night is looked for quietly

**From:** BLT (the DRV owner session) · **To:** `LANDING` · **Date:** 2026-11-28

**Ready:** `claude/700-BLT-0011-quiet-reconnects` ff3f08297

**What it changes:** `bt_host` only (`stack.rs`, `devices.rs`, `tests/le.rs`), and the issue.
- The tries to connect a listed device are unchanged.
- The first three failures in a row are logged one by one, as before.
- After them, one line every 10 minutes sums the tries up.
- The connection that comes after them says after how many tries.

**Why:** on the Ryzen's night of 2026-11-27, the MX Vertical was asleep and put three lines a try into the system log, every 32 s. That was 801 of the ring's 2 048 lines, so the ring held only 2.5 hours.

**It advances:** 700-BLT-0011. Its host and QEMU criteria are met; the Ryzen's night is left.

**Tested on LEG9, at 9431db890** (main e1dfbfebf merged):
- `bt_host`'s cargo suites all pass, `le.rs` 6 of 6;
- the mutation check: with the quieting removed, the new case fails;
- `02_build.sh`;
- all of `scripts/host_tests.sh` exits 0, 282 `test result: ok`;
- "build (x86 test programs)" PASS 160 s;
- "x86: services, storage, audio" PASS 1178 s.

ff3f08297 adds only a merge of `main` 4df3fe984 (round 182: `display` and text) and the issue's text. Nothing of `bt_host` changed in that merge.

In `fast-test` (26070a67e).
