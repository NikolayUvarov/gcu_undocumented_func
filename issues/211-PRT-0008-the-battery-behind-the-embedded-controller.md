# 211-PRT-0008 — The battery and the power source: the Smart Battery System behind the ACPI embedded controller

**Type:** new service `battery`, `libmind` `battery`, `idl/battery.wit`, the kernel's platform ports (`kernel/src/arch/x86_64/platform.rs`) · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress (made, tested on the host and in QEMU; the MacBook Pro's run and the tools track's indicator left) · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-3.3, MC-12.3

## Problem

The maintainer (2026-10-12): "The battery's state must also be shown: on the mains, % left or minutes, switched." Nothing in the system read the battery.

The MacBook Pro's tables, as `log:acpi/` holds them:

- **The embedded controller.** The ECDT names it, with its command/status port at 0x66 and its data port at 0x62.
- **The SMBus host.** In the DSDT, `\_SB.PCI0.LPCB.EC.SMB0` (`_HID` "ACPI0001", `_EC` 0x2010) is an SMBus host in the controller's space at 0x20.
- **The Smart Battery System behind it** (`SBS0`, ACPI0002), read by the DSDT's methods:
  - the battery at SMBus address 0x0B;
  - the manager at 0x0A (BatterySystemState).
- **The power source.** `BAT0` and `ADP1` are control-method devices that run AML, which the system has no interpreter for. Their source of power is a bit of the controller's space (`EACP`).

## Plan

- **`libmind` `battery`**, a model-free library:
  - the ECDT's two ports;
  - ACPI0001's `_EC` found in the AML as bytes;
  - the controller's byte protocol (RD_EC, WR_EC: ACPI 6.5, 12.3);
  - the SMBus host interface in its space (12.9: Read Word, Read Block);
  - the battery's words (Smart Battery Data 1.1) and what they mean.
- **The power source** comes from the charger's AC_PRESENT where the charger follows the specification (bit 4 set, not 0xFFFF). Otherwise it comes from the battery's own status: not discharging means on the mains. MacBooks' chargers answer 0xFFFF, and Linux's `sbs` driver ignores them the same way.
- **The kernel** hands out exactly the two ports the ECDT names (`PLATFORM_PORTS`), beside the platform profile's fixed ones.
- **`init`** finds both while it holds the platform privilege. It then starts `battery` with the two ports and `smbus=0xNN` (211-KRN-0090).
- **The service `battery`** (`idl/battery.wit`):
  - it reads the battery every 5 s, and again before answering if the last reading is older;
  - it only writes the SMBus host's address, command and protocol registers, and waits for the firmware's own transaction to end first;
  - it logs the battery's maker, name and chemistry once, then each change of the power source or of charging, each 5 % of charge, and errors.
- **For the maintainer before the indicator exists:** `diag/quick.msh` and `diag/full.msh` show `dmesg -s battery`.
- **The indicator in `wm`** is the tools track's (`requests-APP.md`): on the mains or not, % left or minutes, switched by a click (the maintainer, 2026-10-12).

## Acceptance criteria

- **Host tests** (`tests/battery_host.rs`, on a model of the controller with an SMBus host and a battery):
  - the ports and the host's base are found in an ECDT and in AML; the MacBook Pro's tables were also checked once, outside the repository: 0x66, 0x62, 0x20;
  - a battery on its own power and on the mains, charging and full;
  - a charger to the specification, and one answering 0xFFFF;
  - a transaction that waits for the firmware's;
  - a missing device, a host that stays busy and a controller that does not answer, told apart.
- **QEMU** (no ECDT): `battery` is not started; the other suites pass.
- **The MacBook Pro:** `dmesg -s battery` names the battery and its charge, and the source follows the adapter being plugged in and out.

## Progress

**2026-10-12.** The host tests pass. QEMU x86: the boot, normal, services, shell, usb and vfs suites pass with `battery` not started.

## Related

[211-KRN-0090](211-KRN-0090-a-slot-for-the-battery.md), [requests-APP.md](requests-APP.md) (the indicator), [docs/profile/bootstrap.md](../docs/profile/bootstrap.md).
