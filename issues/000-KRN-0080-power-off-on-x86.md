# 000-KRN-0080 — Power off on x86 PCs: ACPI S5

**Type:** kernel (`PRT` lines, `kernel/src/arch/x86_64/acpi.rs`) · **Owner:** kernel session · **Priority:** P1 · **Status:** in progress (made; the tests left) · **Blocked by:** — · **Roadmap:** track H · **Constitution:** MC-6.1

## Problem

The maintainer (2026-10-11): there is no shutdown, and it is needed while the system has to be turned off by hand.

- **The shell already has it.** `reboot --off` writes what `vfs_server` caches, stops the boot services through `init`, then asks the kernel to turn the machine off.
- **The kernel can only do it on aarch64** (PSCI `SYSTEM_OFF`, issue 203). On x86 `REBOOT_POWER_OFF` returned `ERR_INVALID`, "no ACPI sleep states yet". So on the MacBook Pro and every x86 PC the system could only be restarted, or turned off by holding the power button.

## Plan

- **At boot the kernel reads from the FADT:**
  - the PM1a and PM1b control blocks (`X_PM1x_CNT_BLK` in I/O space, else `PM1x_CNT_BLK`);
  - `SMI_CMD` and `ACPI_ENABLE`;
  - the DSDT's `\_S5` package: `NameOp`, `_S5_`, `PackageOp`, its length, the count, then SLP_TYPa and SLP_TYPb as Zero, One or a byte.
  - It logs `ACPI: S5 FOUND, POWER OFF POSSIBLE` or `NO S5 IN THE DSDT`.
- **`REBOOT_POWER_OFF`:**
  - stops the other CPUs;
  - turns ACPI mode on through `SMI_CMD` if the firmware left SCI_EN clear;
  - writes SLP_TYP, then SLP_TYP with SLP_EN, to PM1a and PM1b.
  - With no S5 known it still answers `ERR_INVALID`.
- **A machine still on.** The firmware's `_PTS` is not run (there is no AML interpreter). If the machine stays on, the kernel halts it and says on the screen that its disks are written and it may be turned off by hand.
- **The tools track's part** (`requests-APP.md`): `poweroff` and `shutdown` in the shell as names for `reboot --off`, an entry in `wm`'s menu, and the help texts that say "aarch64 only".

## Acceptance criteria

- **QEMU.** On x86 (OVMF), `reboot -f --off` logs `POWER OFF VIA ACPI S5` and QEMU exits; on aarch64 as before.
- **The MacBook Pro.** `reboot --off` turns it off. Alternatively, the boot log names why it cannot: no S5 found, or a halt with the message.

## Related

[issue 084](../issues-done/084-reboot.done) (`reboot`), [issue 203](../issues-done/203-aarch64-smp-and-power.done) (PSCI), [000-KRN-0076](000-KRN-0076-a-frozen-machine-restarts-and-starts-safely.md) (a clean shutdown clears the mark of an unfinished run).
