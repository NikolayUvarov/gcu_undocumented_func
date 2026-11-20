# 352-UPD-0047 — An installer ISO: a live system for x86-64 and aarch64 UEFI, from a disc, a stick or a virtual machine's drive

**Type:** image, script · **Owner:** `UPD` (the kernel session), with `KRN` for the volume in memory · **Priority:** P2 · **Status:** open (plan, 2026-11-20) · **Blocked by:** — · **Main task:** [352](352-installable-media-for-pcs-and-arm-devices.md) · **Constitution:** MC-3.12, MC-9.1, MC-9.2, MC-9.7, MC-12.1, MC-12.3

## Problem

A computer is given MIND Core today only as a raw disk image (`dist/mind-core-usb.img`) that another machine writes ([docs/write-disk.md](../docs/write-disk.md)).

**What the image does not cover:**
- **The usual install medium.** People and hypervisors install an operating system from an ISO: a disc, a stick written by any ISO writer, or a virtual machine's optical drive.
- **Optical drives.** The system reads its boot volume through its disk drivers, which read no optical drive. A system started from a disc could not read its own volume.
- **Running without installing.** The raw image is the installed system. Run from a stick, it writes its log there and its updater fills slot B there.

## Plan

1. **The ISO's layout** (`scripts/make_iso.py`, `06_make_iso.sh`):
   - an ISO 9660 volume with an El Torito UEFI entry, whose FAT image is the boot volume that `make_usb_image.py` builds: the loader and slot A, signed, and `release/` with the release's channel and key file;
   - a hybrid layout, so that the same file written to a stick boots too: an MBR or GPT whose ESP entry points at that FAT image;
   - one ISO per architecture: `mind-core-<version>-x86_64.iso` with `EFI/BOOT/BOOTX64.EFI`, `mind-core-<version>-aarch64.iso` with `BOOTAA64.EFI`.
   - Built with `xorriso` and `mtools`, pinned by version in the build's record (MC-9.7). The script refuses another version with a message naming it.
2. **The volume in memory (KRN).** The bootloader marks a live boot and reads the boot volume's FAT image into memory, checked against its manifest.
   - The kernel hands that memory to an instance of `ramdisk`, read-only, which serves the boot volume.
   - The system then needs no optical or USB driver for its own volume. The image's size limit (RAM) is printed by the script.
   - The handover is a change of `BootInfo` and of `init`'s list, so a `KRN` task (the ABI rule, AGENTS.md section 1).
3. **A live boot writes nothing of its own:**
   - the log goes to memory only, unless a `MIND LOG` partition is found on the stick;
   - the updater stays idle and says why (`[UPDATER] A LIVE SYSTEM: NO SLOT TO UPDATE`);
   - the store runs on `ramdisk#1`, as on a machine without a disk of its own.
4. **The live system starts the installer** of [352-UPD-0048](352-UPD-0048-the-installer-onto-a-disk-of-this-machine.md) on its console, and the shell stays usable for a look first.
5. **Secure Boot:** the ISO carries the same signed loader as the slots ([351-UPD-0012](351-UPD-0012-secure-boot-with-our-own-keys.md)). A machine that accepts only Microsoft's keys does not start it until its owner enrols ours; the docs say so.

## Acceptance criteria

- [ ] **QEMU, x86-64 (OVMF) and aarch64 (`virt`, AAVMF):**
  - the ISO boots as a SCSI or AHCI CD-ROM and, the same file, as a USB disk;
  - the boot volume is served from memory, and the shell lists it;
  - nothing on the ISO's medium is written (its SHA-256 is the same after the run).
- [ ] The ISO's FAT image is the one `make_usb_image.py` built for that commit (the same SHA-256), so one build gives both.
- [ ] A new suite, `iso`, in `tests/qemu_smoke.py` and a group in `scripts/ci_local.sh`, for each architecture.
- [ ] docs/write-disk.md (EN and RU) says how to write the ISO, and which machines it was booted on. A real machine counts only when named (MC-12.1).

## Related

[352](352-installable-media-for-pcs-and-arm-devices.md), [352-UPD-0048](352-UPD-0048-the-installer-onto-a-disk-of-this-machine.md), [351-UPD-0012](351-UPD-0012-secure-boot-with-our-own-keys.md), [211-PRT-0006](211-PRT-0006-log-partition-in-the-image.md) (`MIND LOG`), [docs/write-disk.md](../docs/write-disk.md).
