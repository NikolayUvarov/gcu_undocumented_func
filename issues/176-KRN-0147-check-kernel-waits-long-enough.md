# 176-KRN-0147 — The bench suite gives `check kernel` time to finish

**Type:** test (QEMU) · **Owner:** kernel session · **Priority:** P2 · **Status:** made (2026-10-21); the gate left · **Blocked by:** — · **Main task:** [176](176-test-and-performance-utilities.md) · **Constitution:** MC-12.2

From the tools track's request in [requests-KRN.md](requests-KRN.md) (2026-10-21), for 000-APP-0099, numbered here.

## Problem

- **The wait.** `bench_suite` runs `vm.command("check kernel")` with `command`'s default of 8 s.
- **The kernel group has grown:** text input (000-KRN-0094), every launch slot (000-KRN-0101), stop and notice (800-KRN-0138), and a tool's console closing (176-KRN-0103).
- **Its last check waits for kbench:** up to 10 s for each of its four outputs, and 30 s for it to end.
- **Two runs timed out there:** on `DESKTOP-i7-930-wsl-u24` and on un-rizen ("aarch64: programs, shell and four CPUs"). No row failed.

## Plan

`check kernel` gets 90 s, which covers its slowest check with room for a loaded machine. The full `check` run before it already has 600 s.

## Acceptance criteria

- [ ] The bench suite passes in the gate on x86 and aarch64 with the new wait.
- [ ] It passes on `DESKTOP-i7-930-wsl-u24` (the tools track's next run there).

## Progress (2026-10-21)

- **Local runs,** this session's machine (QEMU, 4 CPUs), branch 3a960da: the `bench` suite passed on x86 and on aarch64, `check kernel` included.
- **Left:** the gate, and the tools track's run on `DESKTOP-i7-930-wsl-u24`.

## Related

[176](176-test-and-performance-utilities.md), [176-KRN-0103](../issues-done/176-KRN-0103-the-tools-stop-when-their-console-is-gone.done).
