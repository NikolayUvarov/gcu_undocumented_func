# 251-APP-0010 — `models:` in the system: the model disk read-only through `vfs_server`

**Type:** tools (services, programs) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [251](251-model-cache-and-model-disk.md) · **Roadmap:** track G · **Constitution:** MC-3.4, MC-4.2, MC-12.1

## Problem

`scripts/models.py disk` writes a FAT32 model disk labelled `MIND MODELS` (251-APP-0009). The system does not show it yet. `vfs_server` mounts only the first FAT volume it finds and the RAM disk. A model disk ahead of the boot disk would even be taken for the boot disk.

## Plan

- **`vfs_server`:**
  - A FAT volume labelled `MIND MODELS` is mounted as `models`, never as the boot disk.
  - Every client's root of `models` is read-only, the user's too (MC-3.4).
  - It logs `[VFS] MOUNTED FAT32 FROM <DEVICE> AS MODELS: (<n> MB, READ-ONLY)`.
- **`mind::fs`:** knows the volume `models`, so `models:path` works in every program.
- **Programs:**
  - `df` and `fsck` include `models:` when it is mounted.
  - A new console program `sha256 <file>...` prints hashes as `sha256sum` does, to check a model against its manifest in the system.
- **`03_run_qemu.sh`:** `MIND_MODELS_DISK=<image>` attaches the image as a read-only VirtIO disk.
  - On x86 the boot disk is on `ata`, so the model disk reaches `vfs_server` through `virtio_blk` with no other change.
  - aarch64 needs a second `virtio_blk` instance (251, step 3).
- **Checks:**
  - A host test (`tests/fat_host.rs`): our FAT code reads an image from `scripts/fat32.py` byte for byte, and its check finds no fault.
  - The QEMU `disk` suite: a model disk on VirtIO next to the boot disk shows as `models:`. `sha256` in the system gives the host's hashes, writes are refused, and `fsck models:` is clean.

## Acceptance criteria

- On x86 (QEMU) with a model disk attached:
  - `vfs_server` mounts it as `models:` beside the boot disk and `ram:`;
  - `df` shows it;
  - `sha256` of a file there equals the host's;
  - `write`, `mkdir` and `rm` there are refused.
- Without a model disk nothing changes: `df` and `fsck` print what they printed before.
- The host tests and the QEMU suites the change touches pass.

## Related

[251](251-model-cache-and-model-disk.md), [251-APP-0009](../issues-done/251-APP-0009-model-cache-on-the-host.done), [066](../issues-done/066-vfs-v2-fat-write.done).
