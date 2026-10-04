# 048 — Block write, `ramdisk`, VFS v2 with directory handles (tools F8)

**Type:** tool · **Owner:** tools track · **Priority:** P2 · **Status:** open · **Roadmap:** track G, T2; C8 for VFS · **Blocked by:** — (036, 044 done)

## Problem

The file system is read-only; tools cannot save.

## Plan

- `BLOCK_WRITE`/`BLOCK_FLUSH` in the drivers (data as sealed read-only memory); `ramdisk` service; VFS v2 on `vfs.wit` with directory handles, FAT write, long names, write order and the boot set protected by policy (decision 1 of the plan: FAT as an export path).

## Acceptance criteria

- Host property tests of the FAT writer checked by `fsck.fat -n`; QEMU suite writes on the RAM disk and a raw FAT image.

## Related

[docs/tools/README.md](../docs/tools/README.md) (plan of the tools track, branch `claude/wizardly-franklin-kec1a9`) F8; [039](039-port-services.md).
