# 000-KRN-0077 — Interrupt storms contained

**Type:** kernel · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** track A · **Constitution:** MC-6.3

## Problem

A device whose interrupt line stays asserted, or whose MSI fires without end, keeps a CPU in the kernel's interrupt path. A driver that has not started yet, a wrong device setting, or a device left running by the firmware can each do it. The kernel counts interrupts per line (`irqs` in the scheduler's accounting) but does nothing about a storm. The CPU's tasks starve, and on the boot CPU, so does the tick that wakes sleepers and ends deadlines.

## Plan

- **Detection.** Per line, the kernel counts interrupts in each tick. A line over a bound is masked at the controller: for example 2000 in one 10 ms tick, far above any device in the suites.
- **What follows.** The kernel logs `IRQ <n> STORM: MASKED` and sends the bound driver an IRQ event that says so; `init` then restarts or quarantines the driver as for a fault. A line with no driver stays masked.
- **Unmasking.** The driver asks for the line again (`IRQ_ACK`); a second storm within a minute keeps it masked until a restart.

## Acceptance criteria

- **A QEMU check.** A test driver that never acknowledges a level-triggered line leaves the system responsive. The shell answers, the line is logged as masked, and the driver is restarted or quarantined.
- **Profile and docs.** The profile names the bound.

## Related

[000-KRN-0076](000-KRN-0076-a-frozen-machine-restarts-and-starts-safely.md).
