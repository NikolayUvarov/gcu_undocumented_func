# 000-KRN-0079 — A drive that comes back is checked to be the same medium before anything is written to it

**Type:** kernel (`vfs_server`) · **Owner:** kernel session · **Priority:** P1 · **Status:** in progress (made; the tests left) · **Blocked by:** — · **Roadmap:** track B · **Constitution:** MC-4 (data integrity), MC-6.3

## Problem

On 2026-10-11 the maintainer took the boot stick out of the running MacBook Pro and wrote a new build onto it on another computer. Then they put it back into the same running system, and only then restarted.

`usb_storage` took it for the old stick ("THE DEVICE IS BACK, THE SAME CAPACITY"). `vfs_server` went on as before ("THE USB DRIVE ANSWERS AGAIN"). The old system wrote its journal onto the new stick, and the new stick's `log:boot0001.log` held 9 lines of the old run.

`vfs_server` also caches FAT and directory sectors, changed ones included. Writing those to a different medium can corrupt it. This time the next boot's `check` found both volumes consistent, but that was luck.

## Plan

- **At start.** `Disk` keeps sector 0 as it read it.
- **When the drive answers again after it did not** (211-KRN-0050), `vfs_server` compares, before it asks anything else, sector 0 and every unchanged cached sector with the medium.
  - **All equal:** it goes on as before.
  - **Any differs:** another medium. The cache is dropped with its changed sectors, which belonged to the old medium. The drive's volumes fail from then on, and the log says `[VFS] THE USB DRIVE CAME BACK WITH ANOTHER MEDIUM: ITS VOLUMES ARE CLOSED, <n> CHANGED SECTORS OF THE OLD ONE NOT WRITTEN TO IT`.
- **To use the new medium,** restart. Mounting it in place is later work.
- **What is not claimed.** A medium rewritten with exactly the same sector 0 and the same cached sectors is taken for the old one. A reproducible build writes the same partition table each time. The other cached sectors, which hold FAT and directory sectors of volumes in use, then tell the media apart.

## Acceptance criteria

- **In the `x86: USB image` group:**
  - the stick is unplugged and a copy with another disk signature put in its place;
  - `vfs_server` logs the other medium, and a program on the disk is refused;
  - the copy is unchanged afterwards.
- **The same stick plugged in again** still "ANSWERS AGAIN".

## Related

[211-KRN-0050](211-KRN-0050-a-program-on-an-unplugged-disk-is-refused-at-once.md) (an unplugged disk), [000-KRN-0078](000-KRN-0078-changed-sectors-written-in-runs.md).
