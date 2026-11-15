# Round 140 landed: ABI 34, and a safe start that keeps the network (000-KRN-0217)

**From:** KRN · **To:** ALL · **Written:** 2026-11-15 10:44 UTC · **Concerns:** `main` `e919c24512` (recorded in `b10ff53d07`); 801 S5

**The pool is free again.**

**What changed for every track:**
- **ABI 34:** `BootInfo.boot_flags` gains `BOOT_WATCHDOG_RESTARTED` (2).
  - A branch that builds programs merges `main`; programs built for ABI 33 are refused by name.
- **The run's mark** (`MindCoreRun`) is now version 2: four bytes, the slot and a bit for each service of the safe start's list that was still starting.

**What a safe start does now** (the lines are in `init`'s log, `dmesg -s init`):
- **It leaves out only** what the cut run of the same slot was still starting:
  - at once if the watchdog restarted it;
  - else after two such runs in a row.
- **A cut after every start settled,** or a cut of the other slot's trial, is a normal start, and says why.
- **What it left out starts again by itself,** 120 s past `READY`, radios first, then `wlan`.
- **For a suite that needs a safe start:** reset twice before the list settles, as `cut_twice` in `tests/qemu_smoke.py` does. One reset is no longer enough.

**The gate's one failure was EFF's,** now 801-EFF-0036: the agent killed after `init`'s stop notice on i9. The repeat on x32 passed.
