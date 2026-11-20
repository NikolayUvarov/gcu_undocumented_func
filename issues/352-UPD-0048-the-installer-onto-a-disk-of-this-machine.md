# 352-UPD-0048 — The installer: MIND Core onto a disk of the machine it runs on, from the ISO or from a card

**Type:** `installer` service, `shell` command · **Owner:** `UPD` (the kernel session), with `PRT` (disk tools, 212) and `KRN` (`init`'s list) · **Priority:** P2 · **Status:** open (plan, 2026-11-20) · **Blocked by:** steps 2 to 4 of [212](212-disk-tools.md) (a service for raw disk changes, the partition table, FAT) · **Main task:** [352](352-installable-media-for-pcs-and-arm-devices.md) · **Constitution:** MC-3.12, MC-9.1, MC-9.3, MC-9.4, MC-12.1, MC-12.3

## Problem

Every MIND Core disk is written by another machine: a Linux or Windows host writes the image ([211-PRT-0001](211-PRT-0001-writer-for-an-internal-disk.md) for an internal disk of that host).

**Where this falls short:**
- **A PC with no other system on it,** booted from the ISO ([352-UPD-0047](352-UPD-0047-an-installer-iso.md)), has no host to write its internal disk.
- **A phone or a board with eMMC** cannot have its eMMC written by a host at all. Its system has to copy itself there, from the card it booted.

## Plan

1. **Which disks.** The installer lists the disks the system's drivers serve, from 212's step 1: bus, size, model and serial number, their partitions.
   - It refuses:
     - the medium it booted from (211-KRN-0012's identity);
     - a disk smaller than the image's layout;
     - a disk with a mounted volume.
   - **A disk that already has partitions** is erased only after a second confirmation, which names them.
2. **The confirmation** is typed at the machine's own console:
   - the disk's model and the last four characters of its serial number, typed back;
   - never from the network, Effector or a file. Installing erases a disk, so it is a person's act (MC-12.3).
3. **What it writes**, through 212's service for raw disk changes:
   - the same partitions as `make_usb_image.py` with `--slots`: the boot volume (ESP), `MIND LOG`, and the store's partition;
   - a GPT on a disk larger than 2 TiB, an MBR otherwise, as the images have today;
   - slot A from the live system's boot volume, its files checked against the release's signed manifest before and after the copy (MC-9.1);
   - slot A marked confirmed, slot B empty, and the release's channel and key file kept, so the updater takes the next release (351);
   - the version floor of the installed release kept, so an older release is refused there afterwards (MC-9.4).
4. **A step that fails** leaves the disk with no ESP. The firmware then does not try to boot a half-written disk, and the installer says which step failed. It is run again from the start (MC-9.3).
5. **Booting the installed disk:**
   - the firmware's removable-media path, `EFI/BOOT/BOOTX64.EFI` or `BOOTAA64.EFI`, as the images have;
   - a named boot entry (`MIND Core`) through UEFI's variables is a step of its own, after the first PCs;
   - on a board or phone, the eMMC is booted by its U-Boot or Tow-Boot, which [352-UPD-0049](352-UPD-0049-images-for-arm-boards-and-phones.md)'s entry names.
6. **The shell's `install` command** shows the list and asks for the confirmation. The live system of 352-UPD-0047 starts it.

## Acceptance criteria

- [ ] **QEMU, x86-64 and aarch64:**
  - from the ISO, MIND Core installs onto an empty VirtIO, AHCI or NVMe disk;
  - the host finds the partition table and the FAT volumes correct (`sfdisk`, `fsck.fat`);
  - started from that disk alone, the system boots slot A confirmed and takes an update into slot B through the updater.
- [ ] **Refusals, in QEMU:** the medium the live system booted from; a disk that is too small; a wrong serial number typed back.
- [ ] **A failure, in QEMU:** the VM stopped at each step of the copy leaves a disk that the firmware does not boot and the installer installs again.
- [ ] **A real PC** installs onto its internal disk from the ISO on a stick, and boots from it. That machine and build are named in the profile (MC-12.1).
- [ ] **On an ARM device with eMMC** (a board or the PinePhone), the system installs from its card onto the eMMC, with 352-UPD-0049's firmware there. Shown on that device (MC-12.1).

## Related

[352](352-installable-media-for-pcs-and-arm-devices.md), [212](212-disk-tools.md), [211-PRT-0001](211-PRT-0001-writer-for-an-internal-disk.md), [351](351-self-update.md), [docs/update/slots.md](../docs/update/slots.md).
