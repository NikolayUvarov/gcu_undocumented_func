# 083 — `format`: a new FAT volume on the RAM disk

**Type:** tool · **Owner:** tools track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** track G, T3 · **Constitution:** Appendix B.6

## Problem

There is no way to empty or re-create a volume ([docs/tools](../docs/tools/README.md) §2.1). Only `vfs_server` holds write-badged block clients, so formatting must be a VFS operation, not a tool writing sectors.

## Plan

- `idl/vfs.wit` 2.3: `format(volume: string<16>, label: string<11>) -> result<_, error>`: `vfs_server` writes a new FAT16 (or FAT12 for small devices) on the volume's block device, flushes, and remounts it. Allowed only to a client whose zone covers the whole volume (the user's badge) and only for the RAM disk (`ram:`); the boot disk is refused (`denied`). Open handles on the volume become stale (`not-found`).
- `format ram: [-l LABEL]`: asks `Format ram: (all files are lost)? [y/N]` and calls `format`; `REQUEST_FILES` (the user's client).
- Host test in `tests/fat_host.rs` (formatted image passes `fsck.fat`); QEMU `disk` suite: files on `ram:` disappear after `format ram:`; `df` shows the new label; `format A:` is refused.

## Acceptance criteria

- The tests above pass; README and `docs/tools` describe `format`.

## Related

[065](../issues-done/065-ramdisk.done), [066](../issues-done/066-vfs-v2-fat-write.done), [068](../issues-done/068-fm-write-df-fsck.done).
