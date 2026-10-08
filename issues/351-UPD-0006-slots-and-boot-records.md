# 351-UPD-0006 — Slots A and B, two boot records, a trial try and the fallback

**Type:** update (bootloader) · **Owner:** `UPD` track (open), with `PRT` and `KRN` for `bootloader/` · **Priority:** P1 · **Status:** open · **Blocked by:** [211-KRN-0012](211-KRN-0012-boot-volume-identity.md) (the bootloader reads its own volume) · **Main task:** [351](351-self-update.md) · **Constitution:** MC-9.1, MC-9.3

Numbered by the kernel session at the maintainer's request (2026-10-08); the track is open.

## Problem

The bootloader loads `kernel.elf` and the services by fixed names from the volume's root, and stops on any error. There is nothing to fall back to.

## Plan

- **Layout.** `\MIND\A\` and `\MIND\B\` each hold a kernel, the boot services and the manifest. `EFI/BOOT/BOOTX64.EFI` (`BOOTAA64.EFI`) stays outside the slots.
- **The image script** makes slot A from the build and leaves B empty. An image without slots, as today, still boots from the root, for the transition.
- **Boot records** `\MIND\BOOT0` and `\MIND\BOOT1`, one sector each: a magic, a sequence number, the slot, the tries left, a confirmed flag and a CRC32.
  - The bootloader takes the valid record with the higher sequence number.
  - A writer always overwrites the other record, so a power cut during a write leaves the previous record valid.
- **Trial.**
  - If the chosen record is unconfirmed with tries left, the bootloader writes the same record with one try fewer before it loads anything, then boots that slot with the trial flag in `BootInfo` (351-KRN-0014).
  - With no tries left, it boots the last confirmed slot.
  - A slot that fails verification (350-UPD-0003) is treated as having no tries.
- **Test hooks:** a QEMU test that boots A, stages B by hand from the host, boots B on trial, and confirms or does not.

## Acceptance criteria

In QEMU on x86 and aarch64:

- a confirmed slot B boots after a trial;
- an unconfirmed slot B falls back to A at the next boot;
- a slot with a damaged file falls back to the other;
- a record torn by a cut write is ignored in favour of the other.

## Related

[351](351-self-update.md), [351-KRN-0014](351-KRN-0014-trial-boot-and-confirmation.md), [351-ASR-0005](351-ASR-0005-power-loss-during-update.md).
