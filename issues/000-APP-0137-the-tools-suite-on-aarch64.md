# 000-APP-0137 — The tools suite on aarch64: the monitors' checks read each architecture's screen

**Type:** tools (tests, `hw`) · **Owner:** tools track (`APP`) · **Priority:** P3 · **Status:** open · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-12.1, MC-12.2

**Recorded by:** the tools track, 2026-11-13, from aarch64 runs of the `tools` suite for 000-APP-0032.

## Problem

- **The gate runs the `tools` suite on x86 alone** (`scripts/ci_local.sh --list`: no aarch64 group holds it), so on aarch64 its checks drifted from what the programs show there, and nothing said so.
- **Found on aarch64** (4 CPUs, in a container, 2026-11-13), each after the last was fixed:
  - `top`: with 30 tasks on aarch64's 28 rows, clock's row and the PID-sorted table's first row fell outside the screen (fixed: 9e8539b8, f98e7a17);
  - `memmap`: the kernel arena's row below the physical map's first screen (fixed: 8e03eca1), and the task window at x86's base, where aarch64's starts at L0 entry 255 (fixed: ab3eb2c6);
  - `hw`: the check expects x86's `+NX`, the IDE controller at `00:01.1` and `IRQ 1` held by `ps2_kbd`, none of which aarch64's `virt` has; and `hw` itself names aarch64's CPU IDs `APIC IDs` and its counter `TSC`.
- **Not reached on aarch64 yet:** the rest of `monitors_check`, and the checks after it, `clipboard_check` among them (000-APP-0032 is checked on x86).

## Plan

- **`hw`:** each architecture's names (MPIDR or CPU IDs, the generic timer's counter for aarch64), host-tested.
- **The checks:** each architecture's expectations in `monitors_check` (the processor's features, the PCI devices and the interrupt lines `virt` has), and the rest of the suite run until it passes on aarch64.
- **Then KRN:** the `tools` suite in an aarch64 group of `scripts/ci_local.sh`, asked in `requests-KRN.md`, so it stays checked.

## Acceptance criteria

- **QEMU:** the `tools` suite passes on aarch64 (4 CPUs) and on x86, the clipboard check included.
- **Host tests:** `hw`'s names on each architecture.

## Related

[000-APP-0032](../issues-done/000-APP-0032-system-clipboard.done) (the clipboard's check, in this suite).
