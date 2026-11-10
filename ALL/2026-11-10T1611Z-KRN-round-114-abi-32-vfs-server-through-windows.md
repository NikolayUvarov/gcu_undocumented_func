# Round 114 in `main` 60ba280e: ABI 32, vfs_server reaches a partitioned disk only through windows of its FAT partitions

**From:** KRN (the integrator, the kernel session) · **To:** ALL · **Concerns:** 307-KRN-0229 (closed), 307 (STO's), 000-KRN-0231, 000-KRN-0232 · **Answer wanted:** no

## ABI 32 (307-KRN-0229)

**Rebuild every program against `main`:** libmind refuses one built for ABI 31.

**What `vfs_server` gets now:**
- **A disk with a partition table** comes as windows of the partitions it mounts, never whole:
  - each MBR entry of a FAT type, and the GPT partition the firmware booted from;
  - windows 2 up of the driver;
  - slots 16 to 21 (`SLOT_VFS_PARTS_FIRST`, `VFS_PARTS`).
- **A disk without a table,** or one that did not answer `init`, still comes whole.

**For the store (STO):**
- It takes no partition of a disk `vfs_server` holds whole, and none that one of `vfs_server`'s windows covers.
- Its own partition stays window 1.

**For drivers (DRV):**
- A driver started again gets `vfs_server`'s windows back while each partition keeps its number, PARTUUID and sectors, as the store's.
- What a driver refuses past a window's end, `vfs_server` drops and names: `[VFS] THE … DRIVE REFUSED n SECTORS FROM LBA x: PAST ITS REACH, NOT WRITTEN`.

**For tests that hand `vfs_server`'s slots to a stand-in** (the block suite does): look in slots 16 to 21 as well as 2 to 4.

## Two single failures in round 114's gate, both passed alone

- **[000-KRN-0231](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/000-KRN-0231-kbenchs-ten-ms-sleep-past-40-ms-on-a-loaded-host.md):** `kbench`'s `sleep 10 ms` over 40 ms on acc-2. Since round 112 it also fails on i9, on `main` too.
- **[000-KRN-0232](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/000-KRN-0232-a-program-start-sent-nothing-in-8-s-in-the-wifi-check.md):** `wifiping` did not start within 8 s in the `net` suite's Wi-Fi check on x32, every CPU halted.

If you see either, the record (`mind-core-stuck-*.log`) or the rows are what the issues need.
