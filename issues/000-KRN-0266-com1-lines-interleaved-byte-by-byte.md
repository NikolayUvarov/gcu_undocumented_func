# 000-KRN-0266 — The kernel's lines and a task's log interleave byte by byte on COM1

**Type:** kernel (`kernel/src/main.rs` `serial_print`, `scheduler.rs` `SYSCALL_LOG`'s mirror) · **Owner:** `KRN` · **Priority:** P2 · **Status:** open (2026-11-20) · **Blocked by:** — · **Main task:** — · **Constitution:** MC-12.1, MC-12.2

## Problem

Round 156's gate went red once on `aarch64: the updater and the modes` (acc-1, candidate `ea5f77604d`). Its `updatestore` suite waited 1500 s for `MIND CORE KERNEL: THE TRIAL BOOT IS CONFIRMED`. COM1 carried:

```
MEMORY MANAGER: MIND CORE KERNEL: THE TRIAL BOOT I6S CONFIR4MED MB HEAP.
```

The trial was confirmed. The shell's `MEMORY MANAGER: 64 MB HEAP.` came in byte by byte between the kernel's bytes, so the line the test looks for never appeared whole.

**Two writers share COM1 with nothing between them** (read in `main`, `934483c3bb`):
- `serial_print` writes byte by byte, with no lock: the kernel's lines through `PanicSerial`, such as `trial::confirm`'s;
- `SYSCALL_LOG` mirrors a task's log byte by byte, from the CPU the task runs on.

On two CPUs their bytes mix.

The rerun of the group alone decides whether the gate stays green. The failure is this, not the candidate's code.

## Plan

1. **One lock for COM1's lines, outside the fatal path.**
   - `serial_print` takes it for its whole text, and the `SYSCALL_LOG` mirror for its whole chunk, with interrupts off while it is held, so that a tick on the same CPU cannot wait on it.
   - A spin with a bound: past it the writer writes without the lock rather than wait, so that a CPU stopped while holding it costs a mixed line, not a stop.
   - The fatal report (`Fatal`, `serial_only`) takes no lock, as now.
2. **The test helpers that wait for a kernel line keep accepting it only whole.** The fix is in the kernel, not in the matcher.

## Acceptance criteria

- [ ] A check that runs a task logging without pause on one CPU while the kernel prints lines on another finds every kernel line whole on COM1 (QEMU, x86 and aarch64, 4 CPUs).
- [ ] The gate's groups pass on both architectures; the kernel stays within `kernel/size-budget.json`.

## Related

[801-UPD-0044](801-UPD-0044-a-trial-the-kernel-stopped-names-its-fault-in-the-fallback.md) (round 156 found it), [000-KRN-0210](../issues-done/000-KRN-0210-the-updater-group-ran-past-its-limit-once.done) (a read the new boot's prompt answered, another way a line was lost).
