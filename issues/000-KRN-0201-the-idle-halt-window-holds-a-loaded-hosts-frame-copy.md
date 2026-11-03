# 000-KRN-0201 — The idle CPUs' HLT check waits out a loaded host's last frame copy

**Type:** test · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress (made; local run, then the gate) · **Blocked by:** — · **Roadmap:** the landing queue · **Constitution:** MC-12.2

## Problem

**What failed.** Round 70's gate failed "x86: x2APIC" once on `acc-2`, and the rerun on `acc-1` passed.
- `smp_suite` reported `CPU 0 did not halt when idle`.
- CPU 0 was at CPL 3, copying a 1280×800 frame (R15 = 0x3E8000 bytes, buffers 4 MiB apart): the compositor drawing the last output.

**Round 65 had the same failure.** Its fix, [000-KRN-0199](../issues-done/000-KRN-0199-checks-that-read-a-result-before-it-is-shown.done), widened the sampling from about 200 ms to 2 s. On a loaded `acc-2` the copy outlasted 2 s too.

## Plan

1. **One window for both checks.** The `normal` suite's check of the boot CPU and the `smp` suite's of each CPU sample `info registers` for `HLT=1` for up to `IDLE_HALT_SECONDS` (10 s).
2. **Only through QEMU's monitor.** A command in the guest would print, and its output would set the compositor drawing again.
3. **What stays caught.** An idle loop that spins never shows `HLT=1`, so the check still fails it.

## Acceptance criteria

- **The `normal` and `smp` suites pass** with the x2APIC kernel, here and in the gate.

## Related

[000-KRN-0199](../issues-done/000-KRN-0199-checks-that-read-a-result-before-it-is-shown.done) (round 65), `tests/qemu_smoke.py`.
