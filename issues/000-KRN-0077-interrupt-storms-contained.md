# 000-KRN-0077 — Interrupt storms contained

**Type:** kernel · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** track A · **Constitution:** MC-6.3

## Problem

A device whose interrupt line stays asserted, or whose MSI fires without end, keeps a CPU in the kernel's interrupt path. A driver that has not started yet, a wrong device setting, or a device left running by the firmware can each do it. The kernel counts interrupts per line (`irqs` in the scheduler's accounting) but does nothing about a storm. The CPU's tasks starve, and on the boot CPU, so does the tick that wakes sleepers and ends deadlines.

## Plan

- **Detection.** Per line, the kernel counts interrupts in each tick. A line over a bound is masked at the controller: for example 2000 in one 10 ms tick, far above any device in the suites.
- **What follows.** The kernel logs `IRQ <n> STORM: MASKED` and sends the bound driver an IRQ event that says so; `init` then restarts or quarantines the driver as for a fault. A line with no driver stays masked.
- **Unmasking.** The driver asks for the line again (`IRQ_ACK`); a second storm within a minute keeps it masked until a restart.

## What the code already did, and the gap (2026-10-24)

- **A line cannot storm.** The interrupt entry masks a device line at the controller as it delivers it: on x86 the PIC line (`context::event`), on aarch64 the GIC's SPI. The line is unmasked only when every driver bound to it has called `IRQ_ACK`, or when its one waiter calls `IRQ_WAIT` again. A level line that stays asserted therefore interrupts no faster than its driver takes it. A driver that never acknowledges leaves the line masked for good. A line without a driver is masked.
- **An MSI can storm.** MSIs and MSI-X vectors are not masked at delivery (`set_irq_masked` ignores lines from 16 on). A device that sends messages without end keeps the boot CPU, where they are aimed, in the kernel's interrupt path, whatever its driver does.
- **So the plan changes** to what the gap needs, without an ABI change:
  - **What counts.** A storm counts the messages that find the last one not yet taken. For a bound driver, that is its pending interrupt; without one, a pending interrupt and no waiter.
  - **When it masks.** At `STORM_LOST` (1000) such messages within `STORM_MS` (100 ms), the kernel masks the vector at the device: the MSI-X entry's vector control, the MSI's mask bit, or, without one, its enable bit. It logs `MSI <line> STORM: …, MASKED UNTIL ITS DRIVER TAKES THE LAST`, at most every 10 s a vector.
  - **When it unmasks.** It unmasks the vector when the driver takes that interrupt, or calls `IRQ_WAIT` or `IRQ_BIND`. Messages then come no faster than the driver reads them, as a line's do.
  - **No permanent mask, no restart by `init`.** A driver that keeps up is never slowed, and a device that does not stop costs no more than its driver's pace.
  - **Where to see it.** `stat irqs` shows such a vector as `MASKED=1`.

The first acceptance criterion (a driver that never acknowledges a level line) holds already, by the line's mask at delivery. It is replaced by the MSI case below (2026-10-24, before any of it was claimed).

## Acceptance criteria

- **A QEMU check (x86).**
  - The test kernel (`bar-move-test`) raises QEMU's `edu` device's MSI from CPU 1 without end, with nothing to take it.
  - The storm is logged and the vector masked.
  - `stat irqs` shows it masked, with a count that no longer grows.
  - The shell answers.
- **Profile and docs.** The profile names the bound.

## Related

[000-KRN-0076](000-KRN-0076-a-frozen-machine-restarts-and-starts-safely.md).
