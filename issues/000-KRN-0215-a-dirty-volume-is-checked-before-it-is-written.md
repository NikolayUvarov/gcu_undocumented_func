# 000-KRN-0215 — A FAT volume left dirty is checked at its mount, and one found damaged is not written

**Type:** `vfs_server` · **Owner:** kernel session · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Roadmap:** storage · **Constitution:** MC-4.8, MC-12.3

From DRV's report of 2026-11-05 (`KRN/2026-11-05T1852Z-DRV-the-ryzens-boot-volume-had-a-fat-cross-link.md` on `internal-dev-messages`).

## Problem

**What DRV found on the Ryzen's stick** (FAT16, 8 KiB clusters), read-only from Linux, after four hung boots, each ended by a reset by hand:
- `/System Volume Information` and `/data/LINK/REPLACE/rtw89_listen1.elf` share clusters. The file is the one the link's `replace` wrote on 2026-11-04 at 06:43 UTC. The directory now reads as thousands of junk entries.
- The volume's dirty bit was set.
- `vfs_server` had logged only `[VFS] MOUNTED FAT16 FROM USB AT LBA 63 (DEVICE WRITABLE)`.

**What `vfs_server` does today:**
- **It marks the volume dirty** (FAT[1]) before its first change, and clean only after a flush of everything else succeeded (175-KRN-0049).
- **It never reads that mark at a mount.** A volume left dirty by a reset, a power loss or another system is mounted writable like a clean one.
- **Its allocator trusts the FAT alone.** It takes any cluster whose FAT entry is 0 (`free_run`).
  - A directory entry may still point at such a cluster:
    - a remove whose FAT sectors reached the medium before its directory sector (the cache writes in LBA order);
    - another system's change cut short.
  - Such a cluster is given to a new file. Both then own it: a cross-link, which every later write through either spreads.
- **`check`** (`vfs.wit`, the shell's `fsck`) finds exactly this ("cross-linked", "runs into a free cluster"), but only when someone runs it.

**What is not known:** how this stick's directory came to point at free clusters, and whether it had anything to do with the hangs (DRV: several boots after 11-04 06:43 went well).
- The threat model's list of what a loss during a flush can leave does not name an entry that points at clusters already freed. The order above allows it.
- DRV kept sectors 0–536 (both FATs, the root directory) from before its repair. Comparing the two FAT copies at those clusters, and reading the directory entry's dates, may tell a FAT update cut short from a directory written without its FAT.

## Plan

1. **At a mount, read the clean bit** of a FAT16/32 volume on a writable device. If it is clear:
   - log `[VFS] FAT<n> AT LBA <start> WAS LEFT DIRTY: CHECKING`;
   - run `check` (read-only), and log its counts and first problem.
2. **Damage that a write could spread** (cross-links, chains that run into a free or invalid cluster, invalid entries): the volume is mounted, but read-only. Writes answer `read-only`, and the log and `fsck` say why. Lost clusters alone (clusters in use that nothing reaches) are no such damage: they are never given out again.
3. **A clean check of a dirty volume** mounts it writable, as today. Its dirty bit stays until the next flush after a change, so a boot still writes nothing.
4. **Tests:**
   - host (`tests/fat_host.rs`): a dirty volume with an entry pointing at a free cluster is refused writes and gives out no cluster; a dirty one with lost clusters only, and a clean one, are written as before;
   - QEMU (`vfs` suite): a boot disk made dirty and damaged by the host before the boot. The log names the check and the first problem, a write in `data/` is refused, and `fsck.fat -n` finds the image unchanged afterwards.
5. **`docs/profile/threat-model.md`:** the power-loss row names the case and the check at the mount. **`docs/profile/evidence.md`** gets the new tests.

**What it does not do:**
- **No repair.** Repair is a later task: it needs a copy of what it changes, and the owner's word.
- **A read-only boot volume** also stops writes to `data/` (settings, the store's keys, the link's files) until it is repaired from another system. Not writing over damage is the purpose (MC-4.8), and the log says so.

## Acceptance criteria

- A dirty FAT16 or FAT32 volume is checked at its mount and the result logged. One with a cross-link or a chain into a free cluster accepts no write, and its clusters are never given out. Shown by host tests that fail without the change.
- The `vfs` suite's damaged boot disk, on x86 and aarch64, logs the check, refuses the write, and passes `fsck.fat -n` unchanged after the boot. Every other suite passes as before (the gate).
- The threat model and the evidence rows say so.

## Related

[175-KRN-0049](../issues-done/175-KRN-0049-fat-clean-only-after-a-good-flush.done) (the dirty and clean marks), [000-KRN-0212](../issues-done/000-KRN-0212-a-remove-rename-or-new-directory-reaches-the-disk.done) (a remove reaches the disk at once), [000-KRN-0124](000-KRN-0124-switching-between-linux-and-mind-core.md) (the Ryzen's boots), 800-LNK-0036 (the writer's kept parts).
