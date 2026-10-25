# 100002 — On a battery: idle CPUs, their frequency, sleep and waking

**Type:** main task (power) · **Owner:** `MOB` track (later), with `KRN` (the scheduler, PSCI) and `DRV` (each driver's suspend and resume) · **Priority:** P2 · **Status:** an intention · **Blocked by:** [100000](100000-a-phone-the-pinephone-first.md) stage 2 (the PMIC, the clock controller) · **Roadmap:** track H · **Constitution:** MC-1.1, MC-5.1, MC-5.3, MC-5.6, MC-6.3, MC-12.1, MC-12.3

**Recorded by:** the kernel session, 2026-10-25, with main task 100000, at the maintainer's request.

## Problem

- **What MIND Core does today:**
  - its CPUs run at the frequency the firmware left;
  - an idle CPU waits in WFI with its tick running;
  - the machine never sleeps.
- **A phone then lasts hours.** The MacBook Pro and the Ryzen would gain the same work.

## Plan

- **Measure first:** the battery's current from the PMIC, at idle with the screen on and with it off.
- **Idle:**
  - PSCI `CPU_SUSPEND` for idle CPUs, with the power states the device tree's `idle-states` names;
  - the tick stopped on idle CPUs where the clocks allow, with deadlines kept (MC-5.6).
- **Frequency:**
  - the CPUs' operating points from the tree, set through the clock controller and the PMIC's regulator;
  - by a policy service outside the kernel (MC-1.1).
- **Sleep:**
  - suspend to RAM, through PSCI `SYSTEM_SUSPEND` where the firmware provides it (to be checked on the A64);
  - a contract of suspend and resume for every driver: DMA stopped, state saved, then restored and reconciled on waking (MC-6.3);
  - waking by the power key, the modem's ring or an alarm.
- **The screen:** off after a stated time; the backlight's level.
- **A budget is stated:** hours at idle with the screen off, as measured, never promised beyond the measurement (MC-12.1).

## Acceptance criteria

- **Measured draws** before and after, stated per configuration.
- **A sleep of an hour,** woken by the key and by a call, with every driver working after it.
- **The deadlines of the clock suite** kept with the tick stopped on idle CPUs, in QEMU.

## Related

[100000](100000-a-phone-the-pinephone-first.md), [100001](100001-telephony-sms-and-calls-through-the-modem.md), [211-PRT-0008](211-PRT-0008-the-battery-behind-the-embedded-controller.md) (the MacBook Pro's battery service).
