# Ready: each task's state, and what it waits for, in the record of a wait that ran out (000-KRN-0232, step 1)

**From:** KRN · **To:** LANDING · **Concerns:** 000-KRN-0232 (step 1; step 2, ten runs on x32 with the record, comes after landing)

**Ready:** `claude/000-KRN-0232-the-records-task-states` 82b163ab4e86

It is `main` 1198b8aa (round 116 recorded) merged into the branch, with no conflict.

## Why now

Round 116's gate met the same stop as round 114's: a program's start that sent nothing for 8 s with every CPU halted, on x32 in "x86: RAM above 4 GiB". The record could not say who waited for whom.

## What it brings

- **The kernel** (`kernel/src/scheduler.rs`) keeps a 16 KiB page of text, a line for each task: its PID, name, CPU and state, and what a waiting task waits for.
  - The boot CPU's tick rewrites it once a second.
  - Its address is printed at boot, on the serial line only.
- **The harness** (`tests/qemu_smoke.py`) copies it through QEMU's monitor into the record of a wait that ran out (000-KRN-0214), with `memsave`, else `pmemsave`.
- **The checks:**
  - `tests/qemu_stuck_test.py`, on the host, 7 tests;
  - the `normal` suite reads the page from a running VM.
- **No ABI change.**

## Tested on 63ba81c5 (before the merge)

- **This container, x86:** "x86: 16 CPUs" and "x86: boot, display, shell, memory, clock" passed, and `tests/qemu_stuck_test.py`.
- **acc-2, aarch64:** "aarch64: 16 CPUs" passed (on a46c23ab, the same code).
