# 352-KRN-0268 — The live system's boot volume in memory: the loader reads the El Torito volume, `ramdisk#2` serves it

**Type:** kernel, loader, `init`, `ramdisk`, `vfs_server` (ABI 36, handoff 36) · **Owner:** `KRN` (the kernel session), for `UPD`'s 352-UPD-0047 · **Priority:** P2 · **Status:** in progress (2026-11-21) · **Blocked by:** — · **Main task:** [352](352-installable-media-for-pcs-and-arm-devices.md) · **Constitution:** MC-3.12, MC-9.1, MC-12.3, MC-12.4

## Problem

[352-UPD-0047](352-UPD-0047-an-installer-iso.md) step 1 made a hybrid installer ISO. In QEMU, booted from a disc, the loader reads the kernel and the services from the El Torito entry's FAT volume and checks them against the manifest. Then the running system has no boot volume:
- no driver of the running system reads a CD drive;
- so programs such as `ls` are not found.

Also, OVMF takes the El Torito entry on a USB disk too, ahead of the MBR, so the loader is given no partition there.

The ABI changes only in `KRN` tasks (AGENTS.md section 1), so this is the kernel track's part of 0047's step 2.

## Plan

1. **The loader** names an El Torito entry: `VOLUME_ELTORITO`, with the entry's number, its start and its size in the medium's blocks.
   - From such an entry it reads the whole FAT volume into memory, through the firmware's block I/O. Its size comes from the FAT boot sector, at most 512 MiB.
   - Its serial line says `BOOT: THE VOLUME READ INTO MEMORY: n KB`, or why not.
   - **Handoff 36:** `BootInfo.live_volume` and `live_volume_len`.
2. **The kernel** lends those pages, read-only, through `PLATFORM_BOOT_VOLUME` (ABI 36). It answers `ERR_NOT_FOUND` on any other boot. Tasks see both fields as 0.
3. **`init`** starts the new instance `ramdisk#2` with that memory object in `SLOT_MEM`, on a live boot alone. Otherwise it says `ramdisk#2 NOT STARTED: NO BOOT VOLUME IN MEMORY: NOT A LIVE BOOT`.
   - `vfs_server` gets `ramdisk#2`'s client first among its block devices.
4. **`ramdisk`,** given a memory object, serves it read-only in place of its own 8 MiB.
5. **`vfs_server`** mounts the boot volume from it, matched by the manifest the loader verified (211-KRN-0012), and names the entry.

## Acceptance criteria

- [ ] **QEMU, x86 (the `iso` suite),** with the ISO as an IDE CD-ROM and as a USB disk:
  - the loader reads the volume into memory;
  - `ramdisk#2` serves it read-only, and `vfs_server` mounts it as the boot volume;
  - programs run from it, and a write is refused;
  - the ISO is unchanged.
- [ ] **Every other boot is as before:** `ramdisk#2` is not started, and the gate's groups pass on both architectures.
- [ ] docs/api, docs/profile/bootstrap.md and docs/write-disk.md (EN and RU) say what changed.

**Not covered:**
- aarch64's ISO (352-UPD-0047);
- a real machine (MC-12.1);
- the updater's live mode (0047 step 3). The ISO is built without slots, so the updater finds no zone.

## Related

[352-UPD-0047](352-UPD-0047-an-installer-iso.md), [211-KRN-0012](../issues-done/211-KRN-0012-boot-volume-identity.done) (the boot volume's identity), [300-KRN-0001](../issues-done/300-KRN-0001-blockstore-at-boot.done) (`ramdisk#1`).
