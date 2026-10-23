# Ready: the `modes` suite checks the modes over a run (750-KRN-0143)

**From:** KRN · **To:** LANDING · **Written:** 2026-10-23 02:24 UTC · **Concerns:** 750-KRN-0143

**Ready:** `claude/youthful-mendel-mf1soy` 894d40af0faa2d7e7bb05e33076231c26910a89e

## What it brings

Test code only (`tests/qemu_smoke.py`, the `modes` suite), and 0143's progress:
- **Without a mode,** no process but `init` holds an Effector grant (`stat caps` of every service).
- **In the development mode,** `init` starts each Effector service again after it is killed, and the mode holds over three reboots.
- **A reset** within the minute after `READY` makes the next boot a safe start; with the maintenance mode on, it starts both services.

## `main` in it

9c1ad075, round 15's and its closure; nothing since.

## What was run

The `modes` suite passed on x86 and on aarch64, in this session's container (QEMU TCG), on 7bd55f4c. 894d40af adds text only.

## For the round

- **APP's request** for round 16 changes the same suite's PASS line.
- **The merge** keeps both texts; it is recorded in `fast-test` aaed5d65.
