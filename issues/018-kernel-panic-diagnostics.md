# 018 — Kernel panic diagnostics

**Type:** robustness · **Priority:** medium · **Status:** open

## Problem

`kernel/src/main.rs` `panic_handler` prints only `KERNEL PANIC` to the serial port and halts all CPUs. The message and the source location are lost, which makes kernel faults hard to analyse (Constitution v1.6, Article 10.4: sufficient evidence for failure analysis). Split from issue 007.

## Plan

- Print `PanicInfo` message and location to the serial port without allocating (the heap may be the cause of the panic).
- Print the CPU number and, if available, the current task PID and name.
- Keep the path lock-free: a panic inside the scheduler lock must not deadlock the output.

## Acceptance criteria

- A deliberate `panic!("test")` in a debug build prints `KERNEL PANIC: test at kernel/src/....rs:LINE:COL CPU=n` on the serial port.
- A QEMU smoke check covers it (behind a test-only feature).

## Related

[007](../issues-done/007-kernel-font-and-primitives.done), [ROADMAP](../ROADMAP.md) — assurance.
