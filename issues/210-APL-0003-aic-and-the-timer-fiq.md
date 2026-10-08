# 210-APL-0003 — Apple's interrupt controller (AIC, AIC2) and the timer's FIQ

**Type:** porting (kernel) · **Owner:** `APL` track (open) · **Priority:** P3 · **Status:** open · **Blocked by:** [210-APL-0002](210-APL-0002-board-from-the-device-tree.md); a Mac with M1 ([issues-human](../issues-human/README.md#4-a-mac-with-apple-silicon-to-test-on)) · **Roadmap:** track H · **Constitution:** MC-1.3, MC-5.1, MC-12.1

Part of main task [210](210-apple-silicon-native.md), plan step 2.

## Problem

The interrupt controller of an Apple Silicon Mac is Apple's AIC (M1, t8103) or AIC2 (M1 Pro, Max and Ultra, M2 and later), not a GIC (per Asahi's documentation and Linux's `apple,aic` and `apple,aic2` bindings):

- device interrupts are read from its event register;
- interrupts between CPUs go through it, or on these cores through Apple's own system registers (Linux's driver calls them fast IPIs);
- the generic timer's interrupt arrives as an FIQ, not an IRQ.

The kernel drives only a GICv3 or a GICv2 (`arch/aarch64/interrupts.rs`), unmasks only IRQs and takes no FIQ. Without the timer's FIQ there is no tick: no preemption and no CPU budgets (MC-5.1).

## Plan

- An AIC and AIC2 driver in the kernel's architecture layer, behind the interface the GIC has: device lines, masks, and the interrupts between CPUs for stop, tick and wake.
- The FIQ vector: the timer (the virtual timer, as now) and the fast IPIs where the core has them; FIQs unmasked where IRQs are.
- Lines from the device tree's `interrupts` cells.
- From documentation only (Asahi's, and the bindings' text), not from Linux's GPL code. Tested on hardware: QEMU has no AIC.
- The kernel's code is `PRT`'s directory: done with `PRT`.

## Acceptance criteria

On an M1 the tick arrives on the boot CPU as an FIQ and preempts a task that never yields; a device's interrupt reaches its driver through the AIC; with [210-APL-0004](210-APL-0004-cpus-through-the-spin-table.md), the interrupts between CPUs work (stop, tick, wake).

## Related

[210](210-apple-silicon-native.md), [203](../issues-done/203-aarch64-smp-and-power.done) (SGIs on the GIC), [docs/profile/aarch64/](../docs/profile/aarch64/README.md).
