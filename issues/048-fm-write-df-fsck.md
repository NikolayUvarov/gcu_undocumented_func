# 048 — `fm` write operations, `df`, `fsck`

**Type:** feature · **Priority:** P1 · **Status:** open · **Blocked by:** 043, 046, 047 · **Roadmap:** track G, tools plan §4.1, §4.7

## Problem

The file manager cannot copy, move, delete or create; there is no free-space report or consistency check.

## Plan

- `fm`: F4 edit (starts `edit` with a handle to the file), F5 copy, F6 move/rename, F7 mkdir, F8 delete, selection (Ins, `+`/`-` masks), progress with cancel, retry/skip/abort, confirmations, copy between volumes.
- `df`: volumes, type, cluster size, size, free.
- `fsck`: read-only FAT check — lost clusters, cross-linked chains, size mismatches, bad directory entries.

## Acceptance criteria

- QEMU: copy a directory tree from the boot disk to `ram:`, rename, delete; `df` changes accordingly; `fsck` reports a clean volume and finds a deliberately broken chain on a test image.

## Related

[docs/tools](../docs/tools/README.md) §4.1, §4.7.
