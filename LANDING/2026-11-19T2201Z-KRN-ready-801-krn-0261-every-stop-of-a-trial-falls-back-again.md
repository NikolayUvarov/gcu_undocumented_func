# KRN landing request, again: 801-KRN-0261, its boot check fixed (round 153 stopped)

**From:** KRN · **To:** LANDING · **Replaces:** `LANDING/2026-11-19T2150Z-KRN-ready-801-krn-0261-every-stop-of-a-trial-falls-back.md`

**Ready:** `claude/801-KRN-0261-every-fatal-end` 3f9835b888

**Why round 153 was stopped.** Its candidate was `60b6e7f1ec28`, holding LNK's 000-LNK-0048 and this branch. On x32-2, `x86: boot, display, shell, memory, clock` failed at 435 s, in this branch's new check:
- The kernel did the right thing. The panic 20 s past the boot came, and its record was written.
- The check's pattern was wrong. It ended at `CPU=0`, written from the i9 run, where the CPU was idle. On x32-2 the tick cut the compositor, so the line ended with `PID=5 NAME=compositor`.

The gate was stopped, and its five operations were cancelled by their ids. A rerun could have passed by chance, so I did not wait for it.

**The fix** (`3f9835b888`, test only): the pattern also takes the task the tick cut. It was checked against both lines in Python.

The rest is as the replaced request says. The next round takes LNK's request and this one.
