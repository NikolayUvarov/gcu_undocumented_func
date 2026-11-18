# 801-KRN-0258 — A trial that hangs before `init` comes back only with a watchdog, and neither bench machine has one

**Type:** kernel (`kernel/src/arch/x86_64/watchdog.rs`, the trial's deadline), bootloader · **Owner:** `KRN` · **Priority:** P1 · **Status:** open (2026-11-18) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) · **Constitution:** MC-9.3, MC-6.1, MC-12.1, MC-12.3

## Problem

The DEXP's first release through Effector (801, 2026-11-17, `main` c6a5d2206c as version 1):
- **What happened:**
  - the agent's `shutting_down` at 21:33:55 UTC, then nothing for 12 hours;
  - at about 09:41 UTC on 2026-11-18 the machine started again from slot A, `THE NEWER RECORD: SLOT B NOT CONFIRMED`;
  - the agent answered the kept command `fell back: slot A runs version 0, not 1; the updater: check: Missing`.
- **Slot B left no trace:**
  - no boot log of its own: the log volume's last boot before is `boot0003.log`, and this one is `boot0004.log`;
  - so it stopped before `vfs_server` mounted `log:`, about a second into the boot, or before the kernel.
- **The loader counted its try,** since it booted A at the next start. So it chose B, and the boot stopped after that.
- **The loader's mark** of how far B got (`MIND/REACHED`, 000-KRN-0211) was written over by the next boot.

**Why nothing restarted it:**
- the trial's deadline (`kernel/src/trial.rs`, 120 s) runs on the kernel's own tick;
- `init`'s 600 s bound runs in `init`;
- the chipset's watchdog is armed only just before `init` starts (000-KRN-0076, 000-KRN-0217);
- the firmware's own UEFI watchdog ends at `ExitBootServices`.

So a stop between the handover and `init`, or one that stops the tick, waits for a person. Neither bench machine has a chipset watchdog today:
- **the DEXP** (Intel N100): `WATCHDOG: NONE (THE LPC BRIDGE'S ACPI I/O IS OFF)`. Its PCH keeps the TCO timer behind the SMBus controller, not the LPC bridge the code reads;
- **the MacBook Pro** (7-series PCH): `WATCHDOG: NONE (NO_REBOOT HELD BY THE FIRMWARE)`.

So 801's S1 to S3, "an update that breaks the machine rolls back by itself", does not hold on either machine for a stop that early. It holds only for what the kernel's tick or `init` sees.

## Plan

1. **The TCO timer of Intel PCHs of the 100-series and later** (the N100's Alder Lake-N among them):
   - its I/O base from the SMBus controller's `TCOBASE` (00:1F.4) and its enable;
   - `NO_REBOOT` where those PCHs keep it.

   Both are to be taken from Intel's published PCH datasheets, with the register names and offsets cited in the code. The boot line names the PCH generation it found.
2. **Armed as early as the kernel can,** on a trial boot at least: before the services start, not only before `init`. Fed from the tick as now.
3. **The firmware's watchdog kept to the handover:** the loader leaves the UEFI watchdog running, or sets one, until `ExitBootServices`, so a stop in the loader is reset by the firmware.
4. **The MacBook Pro:** say in the profile that its firmware holds `NO_REBOOT`, so a trial there that stops before `init` waits for a person; look for another reset path, none promised.
5. **Tests:**
   - host tests of the register reads against a model of the 100-series registers;
   - QEMU's q35 keeps its ICH9 path;
   - on the DEXP: the boot line, then one trial built to stop before `init`, which must come back by itself (MC-12.1). It needs the maintainer's word and someone near the machine the first time.

## Acceptance criteria

- [ ] The DEXP's boot line names its TCO timer, armed.
- [ ] On the DEXP, a trial that stops before `init` is reset and boots slot A by itself.
- [ ] `docs/profile` says, for each bench machine, which stops a trial survives by itself.

## Related

[801](801-the-bench-stays-manageable.md), [000-KRN-0217](000-KRN-0217-a-safe-start-that-leaves-out-only-what-it-suspects.md), [213-KRN-0125](213-KRN-0125-the-amd-fch-watchdog.md), [000-KRN-0211](000-KRN-0211-the-loaders-mark-of-how-far-a-boot-got.md).
