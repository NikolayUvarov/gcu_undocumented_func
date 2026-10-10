# 251-KRN-0072 — Free clusters counted a FAT sector at a time

**Type:** kernel (`vfs_server`) · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress (made and tested on the host and in QEMU; the gate left) · **Blocked by:** — · **Roadmap:** track B · **Constitution:** MC-12.2

## Problem

The storage session's request, from its local gate, for the `disks` check of 251-KRN-0031 and 251-STO-0014:

- **The code.** `Volume::free_clusters` (`vfs_server/src/fat.rs`) asked `fat(cluster)` for every cluster. `fat` read each byte of the entry through the sector cache, so a FAT32 entry cost four sector reads.
- **The scale.** A 256 MiB model disk has 65 527 clusters, so the first `df` after it was mounted did about 262 000 cache reads.
- **The time.** Under TCG on aarch64 that took 2.2 to 9.6 s, depending on where the build's code landed. Over 8 s, the `disks` check's `df` timed out. The branch had given that `df` 60 s for the time being.

## Plan

- **`free_clusters`.** It counts through the same two-sector window as `check` (`entry_at`), so each FAT sector is read once for all its entries.
- **The window.** Walking on, it moves its second sector into its first instead of reading both again. That makes it one read a sector, for `check` too.
- **The host test.** `tests/fat_host.rs` compares the count with the one made entry by entry on FAT12, FAT16 and FAT32.
- **The `disks` check.** Its first `df` gets a command's ordinary 8 s again, and prints how long it took.

## Acceptance criteria

1. The counts match the per-entry ones in the host test (FAT12, 16, 32).
2. The first `df` of the 256 MiB FAT32 volume takes well under a second on aarch64 under TCG.

## Progress

**2026-10-10.**
- **The host tests.** `tests/fat_host.rs` passes, 16 tests.
- **On aarch64.** The `vfs` suite on QEMU aarch64 (TCG, the kernel session's machine) printed "the first df … took 0.48 s".

## Related

[251-STO-0014](../issues-done/251-STO-0014-importing-a-model-disk.done), 251-KRN-0031.
