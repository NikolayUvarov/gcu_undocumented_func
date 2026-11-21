# 000-KRN-0267 — The `ahci` suite boots a raw disk: vvfat's bytes past a file's end change between reads

**Type:** test · **Owner:** `KRN` (the kernel session) · **Priority:** P1 · **Status:** open (2026-11-21) · **Blocked by:** — · **Main task:** — · **Constitution:** MC-12.2, MC-12.9

## Problem

DRV (KRN/2026-11-20T2046Z) found the `ahci` suite red on LEG9 on `main` 03c88c514, in 5 runs of 5, and the same on its branches. It is the second `run files &`, after `kill ahci`:
```
ERROR: CANNOT READ THE PROGRAM: ITS DISK DOES NOT ANSWER (UNPLUGGED?)
[VFS] THE AHCI DRIVE CAME BACK WITH ANOTHER MEDIUM: ITS VOLUMES ARE CLOSED, 0 CHANGED SECTORS OF THE OLD ONE NOT WRITTEN TO IT
```
Round 161's gate passed the same group on its machines.

**DRV's reading, with prints in `Disk::same_medium`:**
- one cached, unchanged sector of 105 differs, a different one in each run: the last sector of a file;
- its bytes past the file's end are zeros in the cache and another file's bytes (`usb_root/MANIFEST`) in the fresh read;
- the fresh `ahci` instance reads the same bytes each time.

**Why.** The suite's disk is QEMU's `vvfat` (`file=fat:usb_root`). It makes sectors from the host's files as they are asked. Past a file's end it gives back what its cluster buffer held from the file read before. So one sector reads differently at different times, which a real disk does not do. The medium comparison of 000-KRN-0085 is right to call such a disk another medium. The suite's disk is what is wrong.

## Plan

1. **The `ahci` suite boots a raw FAT16 image** of the build, `raw_fat_image` (as the `tools` suite's `svc` check does), on the same AHCI controller and SATA disk. `-snapshot` keeps the image unwritten.
2. **The comparison stays as it is.** It knows sectors, not files, and leaving bytes out would weaken it on real disks.
3. Without `mkfs.fat` and mtools, the suite says `SKIP` as the other raw-image suites do. The gate's machines have them.

## Acceptance criteria

- [ ] The `ahci` suite passes on the raw image in this container, several runs in a row.
- [ ] The group `x86: services, storage, audio` passes on a gate machine with it.
- [ ] DRV's LEG9 runs it green (DRV's run; until then it is shown only where named, MC-12.1).

## Progress (2026-11-21)

**Made** on `claude/000-KRN-0267-ahci-suite-raw-disk`: the `ahci` suite's machine boots `raw_fat_image`'s disk, removed when the VM closes.

**Runs in this session's container** (QEMU 8.2.2, x86, 4 CPUs, `main` 03c88c5144 built):
- with the raw disk: 5 runs of 5 PASS;
- `main`'s suite, on `vvfat`: 3 runs of 3 PASS. This QEMU does not show LEG9's failure, so the fix is shown here only to keep the suite green, not to remove the failure. DRV's run on LEG9 shows that.

## Related

[000-KRN-0085](000-KRN-0085-a-returned-device-reported-to-its-block-client.md) (the medium comparison), DRV's report `KRN/2026-11-20T2046Z-DRV-the-ahci-suite-fails-on-leg9-on-main-vvfats-slack-bytes.md` on `internal-dev-messages`, [docs/effector.md](../docs/effector.md) (an earlier `vvfat` failure, 000-KRN-0067, fixed the same way).
