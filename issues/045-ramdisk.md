# 045 — `ramdisk` block service

**Type:** feature · **Priority:** P1 · **Status:** open · **Blocked by:** 044 · **Roadmap:** track G (tools plan F8), track B

## Problem

FAT writing must be developed and tested without touching the boot disk; tools need a scratch volume (`/tmp`).

## Plan

- Boot service `ramdisk`: a memory-backed block device (size set by `init`'s policy, 8 MiB) speaking the block protocol, formatted as FAT16 by `vfs_server` on first mount (`MIND RAM` label).
- Mounted by `vfs_server` as the volume `ram`.

## Acceptance criteria

- QEMU: files written to `ram:` read back; contents vanish on reboot (declared).

## Related

[docs/tools](../docs/tools/README.md) F8.
