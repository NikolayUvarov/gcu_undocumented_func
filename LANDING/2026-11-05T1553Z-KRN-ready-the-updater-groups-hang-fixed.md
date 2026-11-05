# Ready: the updater group's hang, fixed in the test (000-KRN-0210)

**From:** KRN · **To:** LANDING · **Written:** 2026-11-05 15:53 UTC · **Concerns:** 000-KRN-0210 (closes once landed)

**Ready:** `claude/youthful-mendel-mf1soy` a284c2aaefd30862fdf1339ed7aee78eefb66240

It contains `main` as of now (cde4a52c).

## What it brings

- **`tests/qemu_smoke.py`, `update_suite`:** the race that held the x86 updater group past Effector's 3500 s.
  - The reading of `dmesg -s updater` ends on a read that holds `MIND CORE KERNEL: REBOOT VIA`. That is a read whose prompt the new boot answered.
  - `trial_confirmed` reads the trial's confirmation from the console's log, from a mark set before. `install_suite` uses it too.
  - `update_suite` waits for `VERSION FLOOR 1 RAISED TO 6`, logged after the confirmation. It was required 12 ms early once on aarch64.
- **`scripts/serve_release.py`:** the test release server's TLS handshake runs in each connection's own thread, not in `accept`. There, one client that never finished its handshake held every other connection. This is a defect found on the way, not the hang's cause.
- **Tests:**
  - `tests/qemu_trial_test.py`, new, in `scripts/host_tests.sh`;
  - `tests/release_test.py`: one test more, which fails without the server's fix.

## Tested

- **Host, in this session's container:** `tests/qemu_trial_test.py` (4) and `tests/release_test.py` (12).
- **x32-128, lane `KRN`, on e0e3c173, with lane `KRN-2` busy:** `x86: the updater and an update through the link`, 12 runs in a row, all passed (604–627 s). Before the fix the same lane hung in runs 7, 15 and 17. Run 17's record names the cause.
- **acc-1, x86:**
  - on e0e3c173: `the updater and an update through the link` passed (675 s);
  - on 88ea32d9: `the updater and an update through the link` and `network, TLS and Effector` passed.
- **acc-2, aarch64:**
  - on e0e3c173: `the updater and the modes` and `an update through the link` passed;
  - on 88ea32d9: the same two and `network, TLS and Effector` passed.
- **Not run before the gate:** a96d33e2, the floor line's wait, which is one changed wait in `update_suite`.
