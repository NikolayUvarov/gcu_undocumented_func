# 352-UPD-0048 — The installer: MIND Core onto a disk of the machine it runs on, from the ISO or from a card

**Type:** `installer` service, `shell` command · **Owner:** `UPD` (the kernel session), with `PRT` (disk tools, 212) and `KRN` (`init`'s list) · **Priority:** P2 · **Status:** in progress (the service made in 352-KRN-0269, 2026-11-21) · **Blocked by:** [352-KRN-0269](../issues-done/352-KRN-0269-the-installer-service.done) (the service, in `vfs_server`), the shell's `install` command (requested of `APP`, [requests-APP.md](requests-APP.md)) · **Main task:** [352](352-installable-media-for-pcs-and-arm-devices.md) · **Constitution:** MC-3.12, MC-9.1, MC-9.3, MC-9.4, MC-12.1, MC-12.3

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

## Design (2026-11-21), from what 352-UPD-0047 and 352-KRN-0268 now give

A plan to be checked as it is made, not a guarantee (MC-12.3).

**What the live system has:**
- its boot volume in slots, read-only in memory, served by `ramdisk#2`;
- a live updater that fills no slot;
- `vfs_server` holding the write clients of every other disk.

**Where the writing happens.** A change to a disk goes through one service (212 step 2), and `vfs_server` already holds the disks' write clients.
- So `vfs_server` serves a new interface, `idl/install.wit` 1.0, to a client badged `BADGE_INSTALL`.
- `init` mints that client on a live boot alone, for the shell's `install` command.
- The interface's calls:
  - `disks`: each disk `vfs_server` holds, with its bus, its size, and its MBR signature or `EMPTY`. The live medium is marked refused, since it holds the volume the loader verified;
  - `install(disk, typed)`: the steps below, answered with the step that failed, if one did.

**What `install` writes:**
1. **An MBR** (a GPT past 2 TiB, 212 step 3).
2. **Partition 1, the ESP:** the live volume sector for sector, read from `ramdisk#2`, its hidden sectors set to the new start.
   - These are the same bytes the loader verified, so slot A is the release's own files, signed.
   - `make_iso.py` sizes that FAT volume for two slots, so the installed disk has room for the updater's slot B.
3. **`MIND LOG`,** formatted by `vfs_server`'s own FAT writer (`fat::format`) with its label.
4. **The store's partition** with its superblock, as `make_usb_image.py` writes it.
5. **A flush,** then the MBR read back.

The ESP's entry is written last, so a disk cut short has no ESP and the firmware does not boot it (plan step 4).

**What is typed back,** as the drivers say nothing of a disk's model or serial number today (`block.wit` 2.0 has no identity):
- the bus and the size in MiB;
- the first four hex digits of the MBR signature, or `EMPTY`.

A model and a serial number are asked of `DRV` for `block.wit` when the first real machine shows they are needed.

## Progress (2026-11-21)

**The service side is [352-KRN-0269](../issues-done/352-KRN-0269-the-installer-service.done),** on `claude/352-KRN-0269-installer-service`. It follows the design above, with these changes:
- **The store's partition is left blank and named in the new ESP's `store.txt`.** No superblock is written, so the store makes itself there at its first start, by its own layout, and the installer holds no copy of `STO`'s format.
- **`vfs_server` writes the table and `MIND LOG` itself.** It does so with its own FAT code, so 212's service is not needed for this layout.
- **On a live boot, `init` gives the store no disk and `vfs_server` every disk whole.** Without this, an empty VirtIO disk would be the store's.

**Shown so far:**
- the writer on disk images on the host (`tests/install_host.rs`): `fsck.fat` finds the volumes clean, mtools reads the files, and a write cut short at each step leaves no ESP's entry;
- in QEMU, the disks offered on a live boot: the blank disk is offered, and the live medium is refused.

**The shell's `install`** is APP's [352-APP-0157](352-APP-0157-the-shells-install-command.md), in round 167.

**The install in QEMU** (`iso` suite, on `claude/352-UPD-0048-install-in-the-iso-suite`, over round 167's candidate). x86 passed in this session's container (QEMU 8.2.2, OVMF):
- the ISO is booted as a CD-ROM, with the blank 512 MiB disk;
- `install` lists the disk as `ATA 512 EMPTY`; `install 1` takes those words typed back at the console and writes the disk: `INSTALL: DONE ON DISK 1, MBR …: THE ESP AT SECTOR 2048 …`;
- that disk, booted alone, verifies its ESP's manifest (`BOOT: VOLUME MBR PARTITION 1 AT LBA 2048`), and `init` gives the block store its partition 3, which `store.txt` names.

**Found on the way:** `init` looks for the store's partition on no legacy IDE disk (`STORE_DRIVERS` has no `ata`). A system installed on an IDE disk therefore boots with no partition for its store; it says `store.txt NAMES PARTITION …: NO DISK HAS IT`. The check boots the written disk on AHCI, as a PC's disk is. Whether the installer should refuse or warn about an IDE disk is open.

**Left:**
- the same on aarch64 (`VIRTIO 512 EMPTY`);
- the host's check of the table and volumes after the install (`sfdisk`, `fsck.fat`), slot A confirmed and an update into slot B;
- the refusals and the failures in QEMU;
- a real PC;
- an ARM device with eMMC.

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
