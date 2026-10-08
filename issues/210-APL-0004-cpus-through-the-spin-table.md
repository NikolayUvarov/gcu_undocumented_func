# 210-APL-0004 — The other CPUs through the spin table

**Type:** porting (kernel) · **Owner:** `APL` track (open) · **Priority:** P3 · **Status:** open · **Blocked by:** [210-APL-0002](210-APL-0002-board-from-the-device-tree.md), [210-APL-0003](210-APL-0003-aic-and-the-timer-fiq.md) (interrupts between CPUs); a Mac with M1 ([issues-human](../issues-human/README.md#4-a-mac-with-apple-silicon-to-test-on)) · **Roadmap:** track H · **Constitution:** MC-5.1, MC-12.1

Part of main task [210](210-apple-silicon-native.md), plan step 2.

## Problem

The kernel starts the other CPUs with PSCI `CPU_ON` into a trampoline (issue 203). An Apple Silicon Mac has no PSCI (expected: m1n1 provides none). Its device tree gives each CPU `enable-method = "spin-table"` and a `cpu-release-addr`, where the CPU waits for an entry address. An M1's eight CPUs are in two clusters, efficiency and performance cores, with different cluster affinities in their MPIDRs.

## Plan

- Write the trampoline's address to each CPU's release address, clean it to memory and send an event (`sev`). The trampoline as now (the MMU on with the boot CPU's settings), at the exception level the CPU is released at (210-APL-0001).
- The CPUs from the device tree (`reg` as the MPIDR) instead of the MADT, in the same table of 256.
- The tick to the other CPUs as an interrupt through the AIC (210-APL-0003).
- The kernel's code is `PRT`'s directory: done with `PRT`.

## Acceptance criteria

On an M1 all eight CPUs start and run tasks; loops that never yield are preempted on every CPU, as the `busy` and `smp` suites check on QEMU; a panic stops every CPU.

## Related

[210](210-apple-silicon-native.md), [203](../issues-done/203-aarch64-smp-and-power.done), [171](../issues-done/171-limits-from-the-hardware.done) (every CPU the firmware reports).
