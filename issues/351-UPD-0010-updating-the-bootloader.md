# 351-UPD-0010 — Updating the bootloader itself

**Type:** update (boot) · **Owner:** `UPD` track, with `PRT` · **Priority:** P3 · **Status:** open · **Blocked by:** [351-KRN-0022](../issues-done/351-KRN-0022-updater-grants.done) (the updater's grant of the firmware privilege, done; `BootNext` and `BootOrder` from the running system are done in [351-KRN-0027](../issues-done/351-KRN-0027-uefi-variables.done)); [351-UPD-0006](../issues-done/351-UPD-0006-slots-and-boot-records.done) is done · **Main task:** [351](351-self-update.md) · **Constitution:** MC-9.3

Numbered by the kernel session at the maintainer's request (2026-10-08), before the track had an owner.

## Problem

Slots A and B cover the kernel and the services, but there is one `BOOTX64.EFI`. A failed write to it leaves a machine that does not boot. Yet the bootloader changes too: signature checks, records, and the format of `BootInfo`.

## Plan

- Two loaders, `\EFI\MIND\BOOTA.EFI` and `\EFI\MIND\BOOTB.EFI`, each with a UEFI boot entry.
- The updater sets `BootNext` to the new one for one trial boot, through a request to the kernel for UEFI runtime variables (a `KRN` task: the kernel does not call runtime services today). After confirmation it changes `BootOrder`.
- The removable-media path `EFI/BOOT/BOOTX64.EFI` stays as the last resort and is changed last.
- Machines whose firmware ignores `BootNext` keep the single-loader path. The profile says which were tested.

## Acceptance criteria

In QEMU with OVMF, a new loader boots on trial through `BootNext`. A loader that fails leaves the machine on the old one at the next boot.

## Related

[351](351-self-update.md).
