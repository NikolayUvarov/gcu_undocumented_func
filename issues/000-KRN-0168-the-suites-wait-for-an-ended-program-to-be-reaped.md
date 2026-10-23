# 000-KRN-0168 — The suites wait for an ended program to be reaped

**Type:** tests (`tests/qemu_smoke.py`) · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — · **Constitution:** MC-12.2

Found in landing round 25 (2026-10-23).

## Problem

- **The failure.** The `x86: keys, shell, tools` group failed once on rz, in 45 s, at `keys_suite`'s `assert task_rows(vm) == {}`. Its one rerun, alone on the same machine, passed.
- **What `ps` showed.** The log showed `keys EXITED BG 0 …` still in `ps`, after `EXITED. SHELL RESUMED.`
- **Why.** The kernel frees an ended task in `scheduler::reap`, which only the boot CPU's idle loop runs (`kernel/src/main.rs`).
  - The shell and the compositor also ran on CPU 0 and kept it busy.
  - Under QEMU's emulation on a loaded host, the reap came after the check.
  - On i9 the group had passed every round.
- **The ten checks.** Ten checks in the suites take an empty table right after a program ends or is killed.

## Plan

1. **A bounded wait.** `task_rows_reaped`: `ps` again every 0.2 s while the only rows left have `EXITED`, for at most 5 s. Then the same check.
2. **Nothing else is loosened.**
   - A program that has not ended still fails at once.
   - One that stays `EXITED` past 5 s fails as before.

## Acceptance criteria

- [ ] The ten checks use the wait; the `keys` suite and a suite with each other check pass on x86 (QEMU).
- [ ] The next gates show no failure of these checks.

## Related

`kernel/src/scheduler.rs` (`reap`).
