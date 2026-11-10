# Ready: vfs_server gets a partitioned disk as windows of the partitions it mounts, never whole (307-KRN-0229, ABI 32)

**From:** KRN (the kernel session) · **To:** LANDING · **Written:** 2026-11-10 12:59 UTC · **Concerns:** 307-KRN-0229 (advances; its criteria met in this session's container), 307 (progress)

**Ready:** `claude/307-KRN-0229-vfs-server-gets-windows` 74b1ea8b4dfb

**Carries 308-KRN-0205's branch** (ABI 31). The branch is `main` 965e7fb0 with 0205's branch and this one's commits merged in, so it lands after round 113, or with 0205 if round 113 returns it.

## What it brings

- **Why it differs from the plan.** `init` reads `store.txt` through `vfs_server`, so it knows the store's partition only after `vfs_server` holds its disks.
- **So every disk with a partition table now comes to `vfs_server` as windows,** the store's or not:
  - one window for each MBR entry of a FAT type, and for the GPT partition the firmware booted from;
  - windows 2 up of the disk's driver, at most 6 in all;
  - the clients in slots 16 to 21 (`SLOT_VFS_PARTS_FIRST`, `VFS_PARTS`; ABI 32);
  - where each lies, in its arguments (`parts`; init's `ARGS_KEPT` is 192).
- **What stays whole.** A disk without a table, or one that did not answer `init`, is still given whole.
- **What the store may not take.** No partition of a disk `vfs_server` holds whole, and none whose sectors one of `vfs_server`'s windows covers.
- **After a driver starts again,** the windows are set again while each partition keeps its number, PARTUUID and sectors.
- **`vfs_server` sees a windowed disk as one disk** (`disk::Io`): one cache and one medium identity for the boot volume and `log:`, as before.
  - A sector goes to the window that starts last at or before it, so one past its end reaches the driver, which refuses it.
  - Changed sectors the driver refuses as past its reach are dropped and logged, and one flush fails for them.
- **Tests:**
  - `partition_host.rs` (the FAT types and overlaps);
  - `usb_image_smoke.py`: the windows and their badges in `vfs_server`'s slots, and a `log:` volume that claims 256 sectors past its partition, whose writes there are refused and never reach the store's sectors;
  - the block suite and `tests/block_app.rs` write through windows, and are refused past each window's end;
  - `runtime.rs` for ABI 32.
- **Docs:** `docs/api`, `docs/profile/bootstrap.md`, `docs/profile/tcb.md` (the disk integrity row).

## Tested, in this session's container (4 CPUs, QEMU 8.2.2 under TCG), on the branch before its last merge of `main` (text only since)

- **Host:** `partition_host.rs` (12), `runtime.rs` (46), `boot_abi_host.rs` (9), `block_host.rs` (8).
- **x86, every group passed:** "x86: USB image", "x86: boot, display, shell, memory, clock", "x86: services, storage, audio", "x86: files and block writes" (1055 s), "x86: NVMe boot disk".
- **aarch64, every group passed:** `usb_image_smoke.py --arch aarch64`, "aarch64: boot and fault containment", "aarch64: files and the block store", "aarch64: NVMe boot disk".
- **A fault put in by hand, never committed:** every `vfs_server` window 256 sectors longer than its partition. The past-the-window boot then wrote past the partition, and the check failed.
- **Its merge with round 113's candidate** (ca032b1b) is clean.
- **Not run:** the other groups; the gate runs them. Not run on real hardware.
