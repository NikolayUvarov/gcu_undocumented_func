# 352-UPD-0047 — An installer ISO: a live system for x86-64 and aarch64 UEFI, from a disc, a stick or a virtual machine's drive

**Type:** image, script · **Owner:** `UPD` (the kernel session), with `KRN` for the volume in memory · **Priority:** P2 · **Status:** in progress (step 1 made, 2026-11-21) · **Blocked by:** — · **Main task:** [352](352-installable-media-for-pcs-and-arm-devices.md) · **Constitution:** MC-3.12, MC-9.1, MC-9.2, MC-9.7, MC-12.1, MC-12.3

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
   - Written by the script itself, not by `xorriso`, so the gate's machines need no new tool. The FAT image is made with `mkfs.fat` and mtools, as the tests' raw images are. Every date is the build's, so the same build gives the same bytes (MC-9.7). (Changed 2026-11-21, when the writer was made.)
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

## Progress (2026-11-21)

**Step 1 made** on `claude/352-UPD-0047-installer-iso`:
- **`scripts/make_iso.py`:**
  - the boot volume: a FAT16 image of `make_usb_image.py`'s payloads, under the build's signed manifest, at 1 MiB, the build's size plus 32 MiB (64 MiB at least);
  - an ISO 9660 volume with one El Torito EFI entry for it, the volume being the ISO's last extent;
  - an MBR in the system area whose one entry, type 0xEF, is the same volume;
  - `check_iso` reads all of it back before the file replaces anything.
- **`tests/make_iso_test.py`** (in `host_tests.sh`):
  - every payload read back from the volume;
  - the same date gives the same bytes;
  - a volume over 32 MiB is counted as 1, "to the end of the disc";
  - a changed byte in the MBR, the descriptors, the catalog, the root directory or the volume's start is refused.
- **The `iso` suite** (x86, in the group `x86: keys, shell, tools`):
  - written as a USB disk, it boots to the shell and `files` runs from its volume;
  - as an IDE CD-ROM, the loader reads and checks the kernel and the services;
  - the ISO's bytes are the same after both boots.
- **docs/write-disk.md** 1.9 (EN and RU), section 12.

**Runs in this session's container** (QEMU 8.2.2, OVMF, x86, 4 CPUs, `main` 03c88c5144 built):
- the host test: 4 of 4;
- the `iso` suite: PASS;
- two builds 2 s apart: the same SHA-256.

**Found on the way:**
- **OVMF takes the El Torito entry on a USB disk too,** ahead of the MBR. So the loader is given no partition (`BOOT: VOLUME UNKNOWN PARTITION 0 AT LBA 0`). vfs_server then mounts the volume that holds the manifest the loader verified, at the MBR partition's start, as 211-KRN-0012 does for a volume the firmware did not name. Naming the El Torito volume in `BootInfo` is a later step of this task.
- **From a disc, the running system has no volume:** `ls` is not found, as no driver serves the CD drive. This is step 2.

**Step 2, the volume in memory,** is [352-KRN-0268](352-KRN-0268-the-live-volume-in-memory.md) (the kernel track's part, an ABI change): from a disc and from a USB disk, the loader reads the El Torito volume into memory, `ramdisk#2` serves it read-only and `vfs_server` mounts it. Programs then run from a disc too.

**Left:**
- steps 3 to 5;
- aarch64;
- a real machine.

## Related

[352](352-installable-media-for-pcs-and-arm-devices.md), [352-UPD-0048](352-UPD-0048-the-installer-onto-a-disk-of-this-machine.md), [351-UPD-0012](351-UPD-0012-secure-boot-with-our-own-keys.md), [211-PRT-0006](211-PRT-0006-log-partition-in-the-image.md) (`MIND LOG`), [docs/write-disk.md](../docs/write-disk.md).
