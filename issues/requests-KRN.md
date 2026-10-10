# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open (4 requests waiting, 2026-10-10; the TPM's registers became [351-KRN-0052](351-KRN-0052-tpm-registers-from-the-firmware.md); the updater's badged VFS client is done in [351-KRN-0022](../issues-done/351-KRN-0022-updater-grants.done); on the kernel branch, the panic in `awaits_reply` became 171-KRN-0054 and the `devicetree` suite's pacing 210-KRN-0055) · **Recorded by:** the tools track (APP), 2026-10-06

The kernel track numbers its own tasks (`NNN-KRN-MMMM`), so requests from other tracks wait here. The kernel track turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.

## A memory quota for `blockstore` that fits its disk's index

**Recorded by:** the storage session, 2026-10-09, for [251-STO-0013](../issues-done/251-STO-0013-an-index-that-grows-with-the-medium.done) (the speech models of 251-STO-0010).

### Problem

The block store's index now grows with its medium: 56 bytes a slot, room for a block per 8 sectors (`slots_for` in `blockstore/src/store.rs`). `blockstore` allocates the slots at mount and halves them until its memory quota allows.

Its quota is the default 16 MiB (`HEAP_MAX_BYTES`), so the index stays under about 14 MiB, roughly 230 000 blocks of 16 KiB: 3.5 GiB of objects. A model disk of several such models, or a store disk larger than that, would mount with an index too small to hold every block, and mounting refuses such a store whole.

### Plan (a proposal; the kernel track decides)

- In `init`'s quotas, `"blockstore" => Quota { memory_mib: BLOCKSTORE_MEMORY_MIB, ..Quota::default() }`, next to `windows` and `compositor`, with 64 MiB. That is an index of 2^20 slots (56 MiB) and the rest of what it holds now.
- Or a quota computed from the store disk's size, if `init` knows it when it starts `blockstore`.

### Acceptance criteria

On a store disk of 8 GiB, `[BLOCKSTORE] INDEX:` reports the slots `slots_for` asks for, not a halved number.

## QEMU's vvfat crashes in the aarch64 boot suite on `main` at 661147f

**Recorded by:** the storage session, 2026-10-09, from its local gate and runs of `tests/aarch64_smoke.py` on plain `main`.

### Problem

The group "aarch64: boot and fault containment" (`tests/aarch64_smoke.py`) fails because QEMU itself stops:

```
qemu-system-aarch64: block/vvfat.c:2760: handle_renames_and_mkdirs: Assertion `j < s->mapping.next' failed.
```

The test boots from a directory served as `fat:rw:` (vvfat), and the crash comes after `[INIT] READY`. That is when services write to the boot volume (`keystore` makes `system/keystore` and its key on a fresh disk), and in the fault cases while `rtc` restarts.

Measured on this machine, with the build of each tree:
- `main` at 661147f: fails 2 runs in 3.
- `main` at 661147f with `vfs_server/src/fat.rs` as it was before b8b172f (175-KRN-0047…0049): fails 2 runs in 4. So it is not the FAT audit fix.
- The same group passed in the storage branch's gate merged with `main` at a9ac93f, before the kernel branch's 32 commits came in. That was one run, so it does not prove the group was reliable before.

### Plan (a proposal; the kernel track decides)

- Find which of the commits between a9ac93f and 661147f changes the writes vvfat sees, or the timing.
- Either way, QEMU documents vvfat with `rw` as unreliable. The boot suite could boot from a raw FAT image made with mtools, as `tests/boot_slots_check.py` does, and keep vvfat for read-only directories.

### Acceptance criteria

The group passes in repeated runs (say 5 of 5) on `main`.

## Free clusters counted a FAT sector at a time

**Recorded by:** the storage session, 2026-10-10, from its local gate (the branch at 68d3b80 merged with `main` at 8f05728), for the `disks` check of 251-KRN-0031 and [251-STO-0014](../issues-done/251-STO-0014-importing-a-model-disk.done).

### Problem

`Volume::free_clusters` (`vfs_server/src/fat.rs`) calls `fat(cluster)` once per cluster, and each call reads its FAT sector through the cache: a zeroed 512-byte buffer, a search of 64 tags and a copy. A model disk's FAT32 volume of 256 MiB has 65 527 clusters, so the first `df` after it is mounted does that 65 527 times.

Under TCG on aarch64 this took 2.2 s with `main`'s `vfs_server` and 9.0 to 9.6 s with the branch's, whose only difference in the call path is that a function was added elsewhere (`Volume::overwrite`). Every function on the path has the same size in both builds, so the time follows where the code lands. Over 8 s the `disks` check's `df` timed out in two groups ("aarch64: files, network and TLS", "aarch64: NVMe boot disk"). The second `df`, from the counted value, took 0.3 s. The branch gives that `df` 60 s for now.

### Plan (a proposal; the kernel track decides)

Count the free entries of each FAT sector read once: 512 sectors for that volume instead of 65 527 calls. A host test in `tests/fat_host.rs` can compare the count with the per-cluster one on FAT12, 16 and 32.

### Acceptance criteria

The first `df` of a 256 MiB FAT32 volume takes well under a second on aarch64 under TCG, and the counts match the per-cluster ones in the host test.

## A failed flush's error reported again by the next flush that succeeds

**Recorded by:** the storage session, 2026-10-10, from its local gate (the branch at 9200b2c merged with `main` at 917838a), for 211-KRN-0050's check in `tests/usb_image_smoke.py`.

### Problem

`Disk::flush` (`vfs_server/src/disk.rs`) writes the dirty lines in runs. When a run's write fails, it sets `self.failed = true` and returns false, and the lines stay dirty. The next flush writes them and then returns `!take(&mut self.failed) && …`, which is false: it reports the earlier failure although nothing was lost and every line is now on the drive.

`unplugged` in the USB image check (211-KRN-0050) shows it. While the stick is out, the journal's save fails and leaves `failed` set. Once the stick is back and programs start again, the shell's `sync` answers `ERROR: SYNC: I/O ERROR`. Only a journal save that runs between the replug and the `sync` (every 30 s after a failure) clears it.

Measured (x86, `scripts/make_usb_image.py --no-build` and `tests/usb_image_smoke.py`):
- `main` at 917838a passed 2 runs of 2;
- the branch failed 3 of 3, the gate's run included. Every ELF differs because the branch changes `libmind`. The branch's tests on `main`'s `usb_root` passed, and so did the branch with `main`'s `vfs_server` or `init`. So the outcome follows timing, not one service.
- The branch with the patch below passed 2 of 2.

### Plan (a proposal; the kernel track decides)

A run that fails leaves its lines dirty, so they are written again and nothing is lost. The loop's failure should therefore not mark the disk failed; `failed` stays for an eviction whose write fails (`write_line`), which does lose the line:

```diff
-            if self.ask(|device| (device.write(lba, &run) == Ok(end - at)).then_some(())).is_none() { self.failed = true; return false; }
+            if self.ask(|device| (device.write(lba, &run) == Ok(end - at)).then_some(())).is_none() { return false; }
```

### Acceptance criteria

The USB image check passes on the branch and on `main`. A flush after a failed one reports success when it writes every dirty line, and an eviction that failed is still reported.

