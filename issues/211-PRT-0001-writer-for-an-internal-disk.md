# 211-PRT-0001 — The image writer for an internal disk

**Type:** porting (tools) · **Owner:** `PRT` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-12.1

## Problem

`scripts/write_usb_linux.py:40` refuses any disk whose transport is not USB, and `05_write_usb_windows.ps1` refuses any bus but USB. A SATA SSD on an internal port of the Linux machine therefore cannot be written, even when it is a spare disk meant for this. The only way left is a bare `dd`, which skips every safety check.

## Plan

- An option `--internal` lets the writer take a SATA or NVMe disk (`tran` sata or nvme). It keeps every other refusal:
  - a disk that holds `/`, `/boot`, `/boot/efi`, `/usr`, `/var` or `/home`;
  - a disk that holds the image or the repository;
  - LVM, RAID or crypt children, or active swap;
  - a non-512-byte logical sector, or a disk smaller than the image.
- The confirmation names the model and serial number (`lsblk -o MODEL,SERIAL`) and asks for them typed back.
- `--list --internal` lists such disks with model, size and transport.
- `tests/test_usb_writer.py` gains the cases:
  - internal refused without the option;
  - accepted with it;
  - the system disk refused even with it.
- The README section on writing the image says how.

## Acceptance criteria

With `--internal`, an internal SATA disk that holds no system file system is written and verified. Without the option it is refused as today. The system disk is refused in every case. The writer's tests pass.

## Related

[211](211-intel-pc-from-a-sata-ssd.md).
