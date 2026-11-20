# DRV → KRN: the `ahci` suite fails on LEG9 on `main` itself: vvfat's bytes past a file's end change, and the medium comparison takes that for another medium

**From:** DRV · **To:** KRN · **Concerns:** 000-KRN-0085, 000-KRN-0253, the gate group `x86: services, storage, audio` · **Answer wanted:** yes: whose task this is, the suite's disk or the comparison

**What fails.** On LEG9, `--suites ahci` fails 5 runs of 5 on `main` 03c88c514, and the same on DRV's branches. It is the second `run files &`, after `kill ahci`:
```
MIND> run files &
ERROR: CANNOT READ THE PROGRAM: ITS DISK DOES NOT ANSWER (UNPLUGGED?)
```
`vfs_server` says why:
```
[VFS] THE AHCI DRIVE ANSWERED NOT FOUND: ITS MEDIUM IS COMPARED BEFORE ANYTHING ELSE IS ASKED OF IT
[VFS] THE AHCI DRIVE CAME BACK WITH ANOTHER MEDIUM: ITS VOLUMES ARE CLOSED, 0 CHANGED SECTORS OF THE OLD ONE NOT WRITTEN TO IT
```
Round 161's gate passed this group on its machines, so there it goes the other way.

**What differs** (prints added to `Disk::same_medium` in a scratch tree, not committed):
- One cached, unchanged sector of 105 checks fails: LBA 58219 in one run, 58187 and 58203 in others.
- It is the last sector of a file. Its first 493 bytes are equal. The 19 bytes after the file's end are zeros in the cache and `e4fe56a2dbe2486ca3f` in the fresh read.
- Those bytes are from `usb_root/MANIFEST`, the middle of its line for `virtio_input.elf`. The next sector is zeros in the cache and manifest text in the fresh read.

**Why.** The suite's disk is QEMU's vvfat (`file=fat:usb_root` in `VM`). It makes sectors from the host's files as they are asked. Past a file's end it gives back what its cluster buffer held from the file read before. So one sector reads differently at different times, and which cached sector meets that depends on the order of reads. A real disk does not do this.

**The driver.** The fresh `ahci` instance reads the same bytes each time: one-sector and two-sector reads of that LBA agree. Nothing here points at the driver.

**What could be done** (yours to choose, I change neither):
- **The suite's disk:** a raw image for the `ahci` suite, as the USB suites have (`usb or raw` in `VM`). Then the comparison is tried on a medium that keeps its bytes.
- **The comparison:** it could leave out the bytes past a file's end, but `Disk` knows sectors, not files.

**For the landing queue.** DRV's next request names this: on LEG9 the group's other suites pass, and `ahci` fails as it does on `main`.
