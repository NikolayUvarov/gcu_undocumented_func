# 211-KRN-0012 — The boot volume: loaded from the bootloader's own device and named in `BootInfo`

**Type:** kernel (ABI) · **Owner:** `KRN` · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-9.1, MC-11.1

## Problem

Two components choose the boot volume, and they can choose different ones:

- the bootloader opens the first SimpleFileSystem the firmware lists (`bootloader/src/main.rs:151`), not the device it was loaded from (issue 006 noted this);
- `vfs_server` mounts the first FAT volume among the block drivers `init` gives it (`vfs_server/src/main.rs:388-398`).

On a PC with an ESP on another disk, or a USB stick attached, the kernel may be read from one volume while programs and `data/` come from another. A later update of one slot (351) would then be written to the wrong place.

## Plan

- **Bootloader.** Open the SimpleFileSystem of `LoadedImage.DeviceHandle`. From the device path, take the disk's identity:
  - for MBR, the disk signature and the partition's start LBA;
  - for GPT, the disk GUID and the partition GUID.
- **`BootInfo`.** A field for that identity. This is an ABI change: `ABI_VERSION` 4 with an explicit transition (MC-12.4).
- **The kernel checks that the bootloader's `BootInfo` is of its own version**, and stops with a message if not. A mismatched `BOOTX64.EFI` and `kernel.elf` pair fails without a word today.
- **`init` and `vfs_server` mount the volume with that identity.**
  - `vfs_server` has no owner in TRACKS.md, so this task changes it.
  - If no driver shows that volume, `vfs_server` says so instead of mounting another.
- **A test:** QEMU with two FAT disks, where the bootloader's own disk is the second.

## Acceptance criteria

With two FAT disks attached, the system reads its kernel and its programs from the disk it booted from, on x86 and aarch64. A bootloader and kernel of different versions stop with a message.

## Related

[211](211-intel-pc-from-a-sata-ssd.md), [211-DRV-0002](211-DRV-0002-ahci-every-port.md), [351](351-self-update.md), issue 006.
