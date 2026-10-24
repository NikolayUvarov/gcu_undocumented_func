# 351-UPD-0029 — The boot key under the root, in the bootloader

**Type:** update (boot, keys) · **Owner:** `UPD` track, with `KRN`/`PRT` for `bootloader/` · **Priority:** P2 · **Status:** open · **Blocked by:** [351-UPD-0026](351-UPD-0026-root-keys-and-a-signed-key-file.md), [351-UPD-0024](351-UPD-0024-version-floor-in-the-boot-records.md) (the records' format) · **Main task:** [351](351-self-update.md) · **Constitution:** MC-9.5, MC-9.6

Split from [351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md) (the maintainer, 2026-10-23).

## Problem

The bootloader checks the manifest with one boot key built into it. Changing that key still needs a new loader at `EFI/BOOT/`: since [351-UPD-0010](../issues-done/351-UPD-0010-updating-the-bootloader.done) a slot carries its own loader, but the one at `EFI/BOOT/` checks it against the slot's manifest with its own built-in key. A compromised boot key cannot be revoked through the key file the updater reads.

## Plan

- **The bootloader carries the root's public keys and threshold**, built in. It reads the key file from the slot, checks it, and checks the manifest with the boot keys the file names. It parses with `libmind` `release`, shared with the updater.
- **The highest key-file version** is kept in the boot records beside the version floor ([351-UPD-0024](351-UPD-0024-version-floor-in-the-boot-records.md)). A slot whose key file is below it is refused.
- **The updater stages the key file** into the slot with the boot files.
- **The transition** (MC-12.4): while no key file has been seen, the bootloader accepts the built-in boot key. Once one has been, it requires one in every slot.
- The bootloader change is made together with `KRN`/`PRT` (TRACKS.md).

## Acceptance criteria

- QEMU, x86 and aarch64:
  - a slot whose manifest is signed by a boot key its key file names boots;
  - after the boot key is rotated, a slot signed by the old key is refused at boot;
  - a key file not signed by the root threshold stops the boot with `BOOT ERROR: KEYS: <reason>`.
- docs/update/README.md and docs/update/slots.md (with slots_RU.md) give the check.

## Related

[351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md), [351-UPD-0024](351-UPD-0024-version-floor-in-the-boot-records.md), [351-UPD-0026](351-UPD-0026-root-keys-and-a-signed-key-file.md), [351-UPD-0027](351-UPD-0027-key-rotation-and-revocation.md), [351-UPD-0010](../issues-done/351-UPD-0010-updating-the-bootloader.done), [351-UPD-0012](351-UPD-0012-secure-boot-with-our-own-keys.md).
