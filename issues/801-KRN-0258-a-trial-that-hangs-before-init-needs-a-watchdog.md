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

## Progress (2026-11-19)

**Made** on `claude/801-KRN-0258-a-watchdog-for-early-trial-hangs`:
1. **The TCO of the PCHs from Cannon Lake's on** (`kernel/src/arch/x86_64/tco.rs`): the SMBus controller 00:1F.4's `TCOBASE` (0x50, bits 15:5) and `TCOCTL` (0x54, `TCO_BASE_EN` bit 8), `NO_REBOOT` as `TCO1_CNT` bit 0 (`NR_MSUS`). Names, offsets and bits as Intel's register database for the 400 Series Chipset On-Package PCH (615146) gives them, its pages SMBus PCI configuration and SMBus TCO I/O. The PCHs are known by their SMBus controller's ID, as Linux's i2c-i801 lists those with Cannon Lake's TCO; the N100's is 8086:54A3.
   - The count is set and reloaded before `NO_REBOOT` clears, and `NMI_NOW` is never written back.
   - `TCO_EN` in `SMI_EN` is cleared at the PM base the FADT names (its PM1a event block, with the control block 4 bytes on, as Intel lays them out). Where it stays on (`TCO_LOCK`), the boot line says the firmware's SMI is kept.
   - A decode the firmware left off with a base set is turned on, and the boot line says so. A strap that holds `NO_REBOOT`, a base of 0, registers that do not answer: refused, the timer left halted.
   - On the older chipsets' path a `NO_REBOOT` that stays set is now named `NO_REBOOT HELD BY THE BOARD'S STRAP OR ITS FIRMWARE`: Intel ties the bit to a strap of the board (the SPKR pin, sampled at power-on, in its ICH4-M design checklist; the 7-series datasheet was not read), so "the firmware" alone was a guess.
   - The 100 and 200 series (Sunrise Point's kind) keep `NO_REBOOT` behind the P2SB: named and refused, not done. An Intel SMBus controller not listed: its ID named.
   - The boot line: `WATCHDOG: INTEL TCO OF THE ALDER LAKE-M AND -N PCH (SMBUS 8086:54A3, TCO AT 0x…), A RESET AFTER 30 S WITHOUT THE BOOT CPU'S TICK`. ICH9's line is as before.
2. **Armed as early as the kernel can:**
   - the TCO behind the SMBus controller needs only configuration space and I/O ports, so it is armed as the kernel starts, before the ACPI tables, the page tables and the screen: the window from `ExitBootServices` to the kernel's first line on the screen, where the DEXP's screen stayed black, is covered there. Its SMI is quieted once the FADT is read, and the boot line comes with the screen;
   - the others (ICH9's GCS and the FCH are MMIO) right after the page tables and the screen;
   - all before the firmware's services, the frame pool, the CPUs and the scheduler; fed from the tick as before.
3. **A panic on a trial** not yet confirmed restarts the machine (`TRIAL: RESTARTING INTO THE FALLBACK`), the watchdog left running in case the restart does not come. The kernel knows the trial from the handoff's first read, so a panic in the page tables is covered too.
4. **The firmware's watchdog to the handover:** each loader sets it as it starts, to five minutes and its two pauses, whether or not the boot manager did.
5. **Tests:**
   - `tests/tco_watchdog_host.rs`: both paths against a model of the registers, and the early arm without MMIO or the FADT;
   - the `watchdog` suite: the `watchdog-test` kernel on trial stops for good with interrupts off once the watchdog is armed, q35's TCO resets it, slot A boots and says the watchdog ended the last run;
   - the `boot` suite: the `panic-test` kernel on trial panics inside the scheduler lock, restarts, and slot A boots.
6. **Docs:** [docs/profile](../docs/profile/README.md) (the supervision and boot rows, and a row in [evidence.md](../docs/profile/evidence.md)), [slots.md](../docs/update/slots.md), [write-disk.md](../docs/write-disk.md) and their Russian versions. They say which stops a trial survives, and on which bench machine.

**The DEXP's registers, read without a restart** (its hardware report `log:/hw0004.txt` of 2026-11-18, taken with Effector's `collect_file`):
- the eSPI bridge 00:1F.0 is 8086:5481, its offsets 0x40 and 0x44 zero: hence `THE LPC BRIDGE'S ACPI I/O IS OFF`;
- the SMBus controller 00:1F.4 is 8086:54A3, `TCOBASE` 0x00000401 (the TCO at 0x400), `TCOCTL` 0x00000101 (`TCO_BASE_EN` and `TCO_BASE_LOCK` set);
- the FADT's PM1a event block is at 0x1800 (4 bytes), the PM timer at 0x1808, the reset register 0xCF9.

So the path behind the SMBus controller is the one this machine has, and its decode is on. `NO_REBOOT` and the timer's own state are in I/O space, which the report does not read: the boot line will say. The host test's model of the N100 holds these values.

## The DEXP's runs (2026-11-19; MC-12.1: this machine, these boots)

On the maintainer's word of 2026-11-18 («Начни задачу DEXP смогу перезагрузить»), with releases built from round 150's candidate `94dbf2bd42` (this branch's tree), through Effector's `update_agent`:
1. **Release 2, as it is:** downloaded in about 9 minutes, restarted at 11:27:10 UTC, slot B confirmed 35 s later (`updated 2, slot B confirmed`). Its boot line:
   `MIND CORE KERNEL: WATCHDOG: INTEL TCO OF THE ALDER LAKE-M AND -N PCH (SMBUS 8086:54A3, TCO AT 0x400), A RESET AFTER 30 S WITHOUT THE BOOT CPU'S TICK; THE FIRMWARE'S SMI AT THE FIRST EXPIRY KEPT`.
   Criterion 1 is met.
2. **Release 3, with the `watchdog-test` kernel:**
   - the first download was cut by the transport (`GET /api/commands/file: NO ANSWER` after 257 s); the command failed and the machine stayed on slot B, connected;
   - the second was taken, and the machine restarted at 11:47:51;
   - slot A's trial stopped as built: the screen showed `SLOT A ON TRIAL`, the watchdog's line as above and `WATCHDOG TEST: THE TRIAL STOPPED FOR GOOD BEFORE INIT` (the maintainer's photo);
   - **nothing reset it in 20 minutes.** The maintainer restarted it by hand: slot B came back, version 2, and the agent answered `fell back: slot B runs version 2, not 3`;
   - that boot's line has no `IT RESTARTED THE LAST RUN`, so the TCO's second expiry was not seen.

   **Criterion 2 is not met.** The likely cause is in the boot line itself: `TCO_EN` stays set in `SMI_EN` (TCO1_CNT's `TCO_LOCK`, or no PM base read), so the first expiry goes to the firmware's SMI handler, which may reload or stop the timer. Not confirmed: the registers after the first expiry were not read.
3. **What follows:** the PCH's over-clocking watchdog, which resets the platform at expiry with no SMI. On the DEXP the DSDT names it (`CWDT`, `_HID INTC1099`, its register at the PM base + 0x54, 0x1854). It is 801-KRN-0259.

**Left:** step 4 of the plan, another reset path for the MacBook Pro, is not looked for.

**801-KRN-0259's runs (2026-11-19, later):**
- **The cause is read:** release 4's boot line gives `TCO1_CNT 0x1000`, so the firmware set `TCO_LOCK` and `TCO_EN` stays on.
- **Criterion 2 is met by the PCH's over-clocking watchdog:** release 5's trial, stopped before `init`, was reset 30 s later by that watchdog, whose mark the next boot read. The loader booted slot A, and the agent answered `fell back: slot A runs version 4, not 5`. The record is in 0259.

## Acceptance criteria

- [x] The DEXP's boot line names its TCO timer, armed (release 2, 2026-11-19).
- [x] On the DEXP, a trial that stops before `init` is reset and boots slot A by itself (801-KRN-0259's release 5, 2026-11-19: by the over-clocking watchdog, not the TCO).
- [ ] `docs/profile` says, for each bench machine, which stops a trial survives by itself.

## Related

[801](801-the-bench-stays-manageable.md), [000-KRN-0217](000-KRN-0217-a-safe-start-that-leaves-out-only-what-it-suspects.md), [213-KRN-0125](213-KRN-0125-the-amd-fch-watchdog.md), [000-KRN-0211](000-KRN-0211-the-loaders-mark-of-how-far-a-boot-got.md).
