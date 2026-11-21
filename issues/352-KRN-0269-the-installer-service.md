# 352-KRN-0269 — The installer's service: `install.wit` in `vfs_server`, and `SLOT_INSTALL` for the shell on a live boot

**Type:** `vfs_server`, `init`, IDL (`install.wit` 1.0), ABI 37 · **Owner:** `KRN` (the kernel session), for `UPD`'s 352-UPD-0048 · **Priority:** P2 · **Status:** in progress (2026-11-21) · **Blocked by:** [352-KRN-0268](352-KRN-0268-the-live-volume-in-memory.md) (the live volume in memory, ABI 36) · **Main task:** [352](352-installable-media-for-pcs-and-arm-devices.md) · **Constitution:** MC-3.12, MC-9.1, MC-9.3, MC-12.3, MC-12.4

## Problem

[352-UPD-0048](352-UPD-0048-the-installer-onto-a-disk-of-this-machine.md)'s design puts the installer in `vfs_server`:
- `vfs_server` holds the disks' clients;
- on a live boot, the boot volume the loader verified is in memory, served by `ramdisk#2` (352-KRN-0268).

What a live boot is still missing:
- **The store takes a disk.** `init` gives the block store the first blank VirtIO disk, so an empty disk is the store's before anyone can install onto it.
- **No table can be written.** A partitioned disk reaches `vfs_server` only as windows of the partitions it mounts (307-KRN-0229), never whole.
- **No client reaches the installer.** Nothing gives the shell a client for it.

A slot of the shell's is part of the ABI (`common/abi.rs`), so this is a `KRN` task.

## Plan

1. **`vfs_server/src/install.rs`, the writer**, run on a whole disk from the FAT volume at sector 0 of another, in this order:
   - an MBR without the ESP's entry, and no GPT left at either end;
   - the volume sector for sector at 1 MiB, its hidden sectors set, then read back past the cache and compared;
   - `MIND LOG`, 64 MiB, formatted by `fat::format`;
   - the store's partition, 128 MiB, type 0x7F, blank and named in the new ESP's `store.txt` (307-KRN-0177), so the store makes itself there at its first start, by its own layout;
   - the ESP's entry last, then the table read back.

   A disk past 2 TiB is refused: a GPT is needed there (212 step 3).
2. **`idl/install.wit` 1.0**, served by `vfs_server` beside `vfs.wit` to clients badged `mind::fs::BADGE_INSTALL`:
   - `disks`: each whole disk it holds, with its bus, its size, its MBR signature or `EMPTY`, the partitions its MBR lists, the words to type back, and why it is refused (the live medium, too small, past 2 TiB, read-only, a volume mounted);
   - `install(number, typed, erase)`: the writer, once the words typed back match (and `erase` when the disk lists partitions); then the new ESP must hold the manifest the loader verified.
   - A new MBR signature, distinct from every disk's.
3. **`init` on a live boot** (the El Torito volume of 352-KRN-0268):
   - gives the block store no disk or partition: it runs on `ramdisk#1` (352-UPD-0047 step 3);
   - gives `vfs_server` every disk whole;
   - mints the shell's client badged `BADGE_INSTALL` into `SLOT_INSTALL`, for its `install` command and no program (ABI 37).
4. **The shell's `install` command** is `APP`'s (requested): it lists the disks, asks for the words at the machine's own console, never in a remote session, asks again before a disk with partitions, and shows the step that failed.

## Acceptance criteria

- [ ] **Host test `tests/install_host.rs`:**
  - a volume made by `mkfs.fat` and mtools installed onto a blank image: the table read by libmind's reader, the ESP and `MIND LOG` clean by `fsck.fat -n` and read back by mtools, `store.txt` naming partition 3;
  - installed again over an installed disk;
  - a disk too small, past 2 TiB or read-only refused untouched;
  - a write cut short at each step leaves no ESP's entry, and a sector the medium did not keep is found.
- [ ] **QEMU, x86 and aarch64 (the `iso` suite):**
  - on a live boot, `init` gives the store no disk, and `vfs_server` holds the blank disk for the installer and the live medium as refused;
  - every other boot is as before.
- [ ] The gate's groups pass on both architectures.
- [ ] docs/api (ABI 37, `SLOT_INSTALL`), docs/idl, docs/profile/bootstrap.md and docs/write-disk.md (EN and RU) say what changed.

**Not covered:**
- an install run through the shell's command, in QEMU (with `APP`'s command, 352-UPD-0048's criteria);
- a real machine (MC-12.1).

## Related

[352-UPD-0048](352-UPD-0048-the-installer-onto-a-disk-of-this-machine.md), [352-KRN-0268](352-KRN-0268-the-live-volume-in-memory.md), [352-UPD-0047](352-UPD-0047-an-installer-iso.md), [212](212-disk-tools.md), [307-KRN-0229](../issues-done/307-KRN-0229-vfs-servers-clients-bounded-to-its-own-partitions.done) (the windows), [307-KRN-0177](../issues-done/307-KRN-0177-the-stores-own-partition-classified-and-granted.done) (`store.txt`).
