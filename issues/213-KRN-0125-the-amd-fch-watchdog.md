# 213-KRN-0125 — The AMD FCH's watchdog: a frozen Ryzen restarts by itself

**Type:** kernel (`kernel/src/arch/x86_64/watchdog.rs`) · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-6.1, MC-6.3, MC-12.1, MC-12.3

## Problem

000-KRN-0076 arms the Intel chipset's TCO timer, so a frozen Intel PC restarts within 30 s. The Ryzen's boots print `WATCHDOG: NONE (NO INTEL LPC BRIDGE AT 00:1F.0)` (fast-test d0c2dea, 2026-10-20), so a freeze there waits for a person at the power button.

The Ryzen is also where MIND Core is booted once from Linux ([000-KRN-0124](000-KRN-0124-switching-between-linux-and-mind-core.md)). A restart after a freeze brings the machine back to Linux by itself, but only if something restarts it.

The AMD FCH has a watchdog of its own, next to the SMBus controller (1022:790B at 00:14.0 on the Ryzen). It is a control and a count register in the FCH's ACPI MMIO block, enabled through the FCH's power-management registers.

## Plan

1. **Find it.**
   - The FCH is identified by 00:14.0's vendor, device and revision.
   - The register layout comes from AMD's Processor Programming Reference for the family (the FCH's PM and WDT registers).
   - Linux's `sp5100_tco` is a cross-check of the behaviour on the same chips, not a source of code (THIRD_PARTY.md: nothing is copied).
2. **Arm it as 0076 arms TCO:**
   - the decode of its MMIO window and the timer turned on;
   - a count of 30 s, with reset as the action;
   - fed by the boot CPU's tick;
   - stopped before a reboot or power-off.
3. **The firmware may hold it disabled.** Some boards' firmware disables the timer for good. The boot line then says that, and nothing else is changed:

   ```
   WATCHDOG: NONE (AMD FCH: DISABLED BY THE FIRMWARE)
   ```
4. **The boot line on success:**

   ```
   WATCHDOG: AMD FCH, A RESET AFTER 30 S WITHOUT THE BOOT CPU'S TICK
   ```

   With `; IT RESTARTED THE LAST RUN` added when its fired bit survived the reset.
5. **Only the kernel touches it.** The registers are not lent to any task, and the hardware report lists the FCH's state.

## Acceptance criteria

- [ ] Host tests of the sequence against a model of the registers:
  - an FCH that arms;
  - one the firmware disabled;
  - a revision this code does not know, left alone;
  - the stop before a reboot.
- [ ] The QEMU suites pass as before; QEMU has no AMD FCH, so the code finds none.
- [ ] On the Ryzen, with the maintainer:
  - the boot line names the AMD FCH watchdog;
  - the `watchdog-test` kernel's freeze is followed by a restart within the timeout;
  - with 000-KRN-0124, that restart is a boot into Linux.

  Recorded for that machine and its firmware only (MC-12.1).

## Related

[000-KRN-0076](000-KRN-0076-a-frozen-machine-restarts-and-starts-safely.md) (the Intel TCO watchdog, the run mark, the safe start), [000-KRN-0124](000-KRN-0124-switching-between-linux-and-mind-core.md), [213](213-amd-ryzen-pc.md).
