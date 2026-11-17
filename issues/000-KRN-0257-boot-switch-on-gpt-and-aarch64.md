# 000-KRN-0257 — `boot_switch.py` on a GPT disk shared with Linux, and on aarch64

**Type:** script (`scripts/boot_switch.py`), docs · **Owner:** `KRN` · **Priority:** P1 · **Status:** open (2026-11-17) · **Blocked by:** — · **Main task:** — · **Constitution:** MC-6.1, MC-9.3, MC-12.1

## Problem

MOB's ask of 2026-11-16 (ask 2), for the maintainer's word of 2026-11-17: Ubuntu 26.04 arm64 and MIND Core on one GPT disk, Ubuntu first, MIND Core started from it by `BootNext` as on the Ryzen ([000-KRN-0124](000-KRN-0124-switching-between-linux-and-mind-core.md)).

`scripts/boot_switch.py` today:
- takes an MBR disk only: `mbr_identity` and `entry_for` read the disk's signature, `MIND CORE` as partition 1 and `MIND LOG` as 2;
- names `\EFI\BOOT\BOOTX64.EFI` only;
- refuses a disk with anything mounted, so a disk holding Linux's root is always refused.

## Plan

1. **GPT.** The entry for a GPT disk is `HD(n,GPT,<PARTUUID>,…)`, matched and made by the partition's number and unique GUID, as `efibootmgr` shows it.
2. **By label, at any number.** `MIND CORE` and `MIND LOG` are found by their FAT volume labels, at any partition numbers, on MBR as on GPT; their GPT names are read where they have them.
3. **The disk with Linux's root.** Allowed, as long as neither `MIND CORE` nor `MIND LOG` is mounted: only those two partitions are refused mounted, and `logs` mounts `MIND LOG` read-only as now. Nothing else on the disk is read or written.
4. **aarch64.** `\EFI\BOOT\BOOTAA64.EFI` when Linux runs on aarch64 (`platform.machine()`), else `BOOTX64.EFI`.
5. **Unchanged:** BootOrder is never written; `once` sets `BootNext` only.
6. **Tests:** `tests/boot_switch_test.py` with `efibootmgr` and `lsblk` outputs of a GPT disk with Ubuntu's root on it and the two partitions at numbers 3 and 4, on x86 and aarch64; the MBR cases as before.
7. **Docs:** the script's own help and `docs/write-disk.md`'s section on the switch, in both languages where it has two.

## Acceptance criteria

- [ ] Host tests as above.
- [ ] The docs.
- [ ] On a machine: MOB's disk on the ARM computer (MC-12.1).

## Related

[000-KRN-0124](000-KRN-0124-switching-between-linux-and-mind-core.md), [000-KRN-0211](000-KRN-0211-the-loaders-mark-of-how-far-a-boot-got.md), [211-KRN-0256](211-KRN-0256-the-log-volume-on-a-gpt-boot-disk.md).
