# 046 — `vfs_server` v2: `vfs.wit`, directory handles, FAT write

**Type:** architecture · **Priority:** P0 · **Status:** open · **Blocked by:** 038, 044 · **Roadmap:** C8 (VFS port), tools plan F8 · **Constitution:** MC-2.3, MC-3.3, MC-3.4, Article 4, Appendix B.6, MC-12.3

## Problem

VFS is read-only, addressed by global paths over a numeric protocol; Cyrillic long names show as `?`; there are no times or attributes; one volume.

## Plan

- `idl/vfs.wit`: `open_dir`, `open`, `read`, `write`, `truncate`, `close`, `stat`, `list`, `create`, `mkdir`, `remove`, `rename`, `volume`, `flush`. Handles are session numbers bound to the client's PID and to the endpoint badge they were opened through.
- **Directory handles:** every path is relative to a directory handle; `..` above it is refused; a handle opened read-only cannot produce a writable child (MC-3.4). Root handles are given by badge: `init` mints a read-only root badge for applications and a read-write badge limited to `/data` and `ram:` for the shell; boot files (`EFI/`, `*.elf` in the root) are never writable.
- **FAT write:** cluster allocation and release for FAT12/16/32, every FAT copy, FSInfo, long names (UTF-16 ↔ UTF-8, checksum, unique `~N` alias), times from the RTC date, directory growth, the fixed root of FAT12/16, the dirty bit in FAT[1]; order data → FAT → directory entry; write-back cache flushed on `flush`, `close` and shutdown.
- Several volumes: the boot disk (`/`) and `ram:` (045).
- `mind::fs` v2 (`Dir`, `File::create`, `write`, `rename`, `remove`, `metadata`); old calls kept as wrappers.
- Power-loss outcome at each step written in the profile (no atomicity beyond FAT).

## Acceptance criteria

- Host property tests of the FAT writer against `fsck.fat -n` and mtools on generated images (FAT12/16/32).
- QEMU: create, write, rename, remove, mkdir on `ram:` and on a raw FAT disk; reread after reboot; `fsck.fat -n` clean; negative tests for `..`, read-only handles and boot files.

## Related

[docs/tools](../docs/tools/README.md) F8; [docs/idl](../docs/idl/README.md).
