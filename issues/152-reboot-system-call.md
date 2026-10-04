# 152 — `REBOOT` system call

**Type:** kernel · **Owner:** kernel track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** track G, T3

## Problem

`HALT` stops all CPUs; nothing restarts the machine (084).

## Plan

- `SYSCALL_REBOOT` (process-control privilege): stops the other CPUs, then resets through the ACPI reset register (FADT) when present, else the PCI reset control port 0xCF9, else the 8042 pulse (0x64 ← 0xFE), else a triple fault; the kernel validates nothing from the caller (no arguments).
- `docs/api`, `docs/profile` (TCB, MC-10.2 table) updated.

## Acceptance criteria

- QEMU: after `REBOOT` the firmware boots the image again (`services` suite through 084, or a kernel test); a task without process control gets `ERR_RIGHTS` (`isolation` case `k`).

## Related

[084](084-reboot.md).
