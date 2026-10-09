# 212 — Disk tools: partitions and file systems on real disks (plan)

**Type:** main task (plan) · **Owner:** `PRT`, with `KRN` (the service side) and `APP` (the commands) · **Priority:** P3 · **Status:** open (a plan) · **Blocked by:** [211-KRN-0012](../issues-done/211-KRN-0012-boot-volume-identity.done) (which disk is which) · **Roadmap:** track H · **Constitution:** MC-3.4, MC-12.1

## Problem

MIND Core can format only its RAM disk (`format`, issue 083). It cannot list the partitions of a disk, change a partition table or make a file system on a real disk. Disks are prepared on another computer (`05_write_usb_linux.sh`). An installation onto a machine's own disk, and a store with a disk of its own (`requests-KRN.md`, "A durable disk for the block store"), both need these tools on the target.

## Plan (steps, each its own task when it is started)

1. **List disks and partitions, read only.** Every block device with its bus, size and model, and the MBR or GPT entries on it: type, start, size, label, and whether it is mounted.
2. **A service for raw disk changes.** Applications get no raw block client. A change to a disk goes through one service, which:
   - refuses a disk with a mounted volume;
   - refuses the boot disk, unless that is asked for explicitly;
   - asks the user to confirm, with the disk's identity.
3. **Partition tables.** Write an MBR or a GPT with given entries, and add or remove an entry.
4. **File systems.** Format a partition as FAT16 or FAT32 with a label, as `vfs_server`'s `format` does for the RAM disk today.
5. **Mounting.** Mount a newly made FAT volume without a reboot.

Each step is tested in QEMU on blank disk images that the host then checks with `fsck.fat` and `sfdisk`/`gdisk`.

## Acceptance criteria

- A blank disk in QEMU is given a GPT with one FAT32 partition from the shell. The host's tools find both correct. The volume mounts.
- The boot disk and a disk with a mounted volume are refused without an explicit confirmation.

## Related

[211-PRT-0006](211-PRT-0006-log-partition-in-the-image.md) (a partition made at build time), [211-PRT-0001](211-PRT-0001-writer-for-an-internal-disk.md), issue 083, [requests-KRN.md](requests-KRN.md).
