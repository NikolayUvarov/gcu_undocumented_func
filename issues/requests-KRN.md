# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open (4 requests waiting, 2026-10-10; QEMU's vvfat crash in the aarch64 boot suite became 000-KRN-0067; the updater's badged VFS client is done in [351-KRN-0022](../issues-done/351-KRN-0022-updater-grants.done), a flush's stale failure in [211-KRN-0068](../issues-done/211-KRN-0068-a-flush-after-a-failed-one.done); the microcode's VFS client became [550-KRN-0061](550-KRN-0061-bcm-wifi-reads-its-microcode.md); the TPM's registers became [351-KRN-0052](351-KRN-0052-tpm-registers-from-the-firmware.md), the toolchain installed once [000-KRN-0060](000-KRN-0060-toolchain-once-before-the-parallel-build.md), `bcm_wifi` as a boot service [550-KRN-0059](../issues-done/550-KRN-0059-bcm-wifi-at-boot.done), and the tools track's `SLOT_SHELL` and `SLOT_CLIPBOARD` from its branch [211-KRN-0058](211-KRN-0058-slots-for-the-shell-and-the-clipboard.md)) · **Recorded by:** the tools track (APP), 2026-10-06

The kernel track numbers its own tasks (`NNN-KRN-MMMM`), so requests from other tracks wait here. The kernel track turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.

## Free clusters counted a FAT sector at a time

**Recorded by:** the storage session, 2026-10-10, from its local gate (the branch at 68d3b80 merged with `main` at 8f05728), for the `disks` check of 251-KRN-0031 and [251-STO-0014](../issues-done/251-STO-0014-importing-a-model-disk.done).

### Problem

`Volume::free_clusters` (`vfs_server/src/fat.rs`) calls `fat(cluster)` once per cluster, and each call reads its FAT sector through the cache: a zeroed 512-byte buffer, a search of 64 tags and a copy. A model disk's FAT32 volume of 256 MiB has 65 527 clusters, so the first `df` after it is mounted does that 65 527 times.

Under TCG on aarch64 this took 2.2 s with `main`'s `vfs_server` and 9.0 to 9.6 s with the branch's, whose only difference in the call path is that a function was added elsewhere (`Volume::overwrite`). Every function on the path has the same size in both builds, so the time follows where the code lands. Over 8 s the `disks` check's `df` timed out in two groups ("aarch64: files, network and TLS", "aarch64: NVMe boot disk"). The second `df`, from the counted value, took 0.3 s. The branch gives that `df` 60 s for now.

### Plan (a proposal; the kernel track decides)

Count the free entries of each FAT sector read once: 512 sectors for that volume instead of 65 527 calls. A host test in `tests/fat_host.rs` can compare the count with the per-cluster one on FAT12, 16 and 32.

### Acceptance criteria

The first `df` of a 256 MiB FAT32 volume takes well under a second on aarch64 under TCG, and the counts match the per-cluster ones in the host test.
