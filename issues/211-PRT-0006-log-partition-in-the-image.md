# 211-PRT-0006 — A log partition in the disk image that every computer reads

**Type:** porting · **Owner:** `PRT` · **Priority:** P1 · **Status:** in progress · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-10.6, MC-12.1

## Problem

A machine without a serial port leaves no record of a run, only what a photo of its screen catches. The image has one partition, the boot volume, whose MBR type is the EFI system partition's (0xEF). Linux mounts it, but Windows and macOS do not mount it by themselves. So files written in its `data/` directory are hard to read on the computer the maintainer reports from.

## Plan

- **The partition.** `scripts/make_usb_image.py` appends a second partition after the boot volume, at the next MiB:
  - 64 MiB, FAT16, MBR type 0x0E (FAT16 LBA), labelled `MIND LOG`;
  - a `README.TXT` saying what is kept there.

  Windows, macOS and Linux mount and write such a partition.
- **The check.** `check_image` checks the partition's place, type, geometry and label.
- **The system's side** is [211-KRN-0019](211-KRN-0019-boot-logs-on-the-log-partition.md): `vfs_server` mounts the partition as `log:` and writes each boot's system log there.
- **Tests.** `tests/usb_image_smoke.py` boots a copy of the image without a snapshot, on x86 and aarch64. Afterwards it reads the boot log and a file written on `log:` on the host with mtools, and checks the volume with `fsck.fat`.

## Acceptance criteria

- The image of x86 and of aarch64 has the partition; `fsck.fat` finds the volume clean.
- The USB image test passes on both architectures.
- A boot log written on the MacBook Pro of [211-PRT-0004](211-PRT-0004-first-run-on-an-intel-pc.md) is read on another computer.

## Related

[211-KRN-0019](211-KRN-0019-boot-logs-on-the-log-partition.md), [211-PRT-0001](211-PRT-0001-writer-for-an-internal-disk.md), [212](212-disk-tools.md).
