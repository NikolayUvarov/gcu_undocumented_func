# 211-DRV-0009 — `virtio_blk` drives every VirtIO disk

**Type:** driver · **Owner:** `DRV` (open) · **Priority:** P3 · **Status:** open · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-9.1

## Problem

Since [211-KRN-0012](../issues-done/211-KRN-0012-boot-volume-identity.done), `vfs_server` mounts only the volume the bootloader was read from. On aarch64 (QEMU `virt`), disks are VirtIO, and `init` starts one `virtio_blk` on the first VirtIO block device only.

With another disk ahead of the boot disk, `vfs_server` sees only the other one, mounts nothing as the boot volume, and programs cannot be loaded (`tests/aarch64_smoke.py`, the decoy test). The block store's own disk (`requests-KRN.md`, "A durable disk for the block store") needs a second VirtIO disk as well.

## Plan

- `init` starts an instance of `virtio_blk` for each VirtIO block device, up to the number of block client slots, as it does for `virtio_net#1`.
- `vfs_server` gets a client of each and finds the boot volume by its identity.

## Acceptance criteria

`tests/aarch64_smoke.py`: with the decoy disk ahead of the boot disk, `vfs_server` mounts the boot disk's volume and a program starts from it.

## Related

[211-KRN-0012](../issues-done/211-KRN-0012-boot-volume-identity.done), [requests-KRN.md](requests-KRN.md).
