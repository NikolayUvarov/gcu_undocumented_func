# 000-KRN-0199 — Checks that read a result before a loaded host shows it: the colour after `fg`, a program's row after it ended

**Type:** tests · **Owner:** `KRN` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — · **Roadmap:** the landing queue (AGENTS.md section 4) · **Constitution:** MC-12.2

## Problem

In round 64 (candidate fbda1685), the group "x86: no PIT", the `normal` suite, failed on both lanes of `un-rizen-service`. The machine ran another track's suites and two gate lanes, with a load average near 58 on 16 cores. The group passed alone on `acc-1`.

- **On lane 2:** `fg must preserve state, not restart`. The suite reads the centre pixel at once after `FOREGROUND PID=3`. On a loaded host the compositor shows the program's frame later, and the pixel was still the other copy's colour. The two checks before it read the same way, one after a sleep of 150 ms.
- **On lane 1:** `assert set(task_rows(vm)) == {1, 3, 4, 5}` after `PID=2 EXITED. SHELL RESUMED.`. `ps` listed the ended program as `29 clock EXITED`: the boot CPU reaps a program that ended when it is idle, later under load (000-KRN-0168). `task_rows_reaped` waits for that, and the other suites use it already.
- **The same reading** follows a program's fault in the `isolation` suite (`{1}` after a fault), which has not failed yet.

## Plan

- The three colour checks wait up to 5 s for the colour they want (`center_pixel_soon`), as the display check does already. A program that restarted, or input that did not reach it, shows a wrong colour that never changes, so the check still fails then.
- The four row checks after a program ended or was killed use `task_rows_reaped`.

## Acceptance criteria

- [ ] The `normal` and `isolation` suites pass on x86 with the waits, in QEMU locally and in a landing round's gate.
- [ ] Nothing waits where a wrong result could still turn right: each wait is for a colour, or for a row of a program that has ended.

## Progress (2026-10-31)

- `tests/qemu_smoke.py`: the three colour checks of the `normal` suite and its two row checks; the `isolation` suite's two row checks after a fault.
- The `normal` and `isolation` suites passed locally (x86, QEMU TCG, 4 CPUs).

## Related

[000-KRN-0168](../issues-done/000-KRN-0168-the-suites-wait-for-an-ended-program-to-be-reaped.done), [000-KRN-0198](../issues-done/000-KRN-0198-two-waits-of-the-x86-suites-short-for-a-loaded-host.done).
