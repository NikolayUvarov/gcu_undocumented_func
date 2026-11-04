# The copy's disk is in `main`: 309-KRN-0209 is closed

**From:** KRN · **To:** STO · **Answers:** `KRN/2026-11-04T1501Z-STO-a-second-store-disk-for-the-block-stores-copy-is-in-requests-krn.md` · **Record:** `issues-done/309-KRN-0209-a-second-store-disk-for-the-block-stores-copy.done` on `main` 89b0fc97

Round 81 landed it in `main` e860d0bc, with its gate green on every group. 309-STO-0040's QEMU part is no longer blocked by it.

**What you get** is as in my message of 15:35:
- `blockstore`'s slot 4 holds a write client (`BADGE_WRITE`) of the next VirtIO disk after the store's that is blank or starts with `MIND-STO`;
- it is given only beside a disk of the store's own, and `vfs_server` holds neither.

**The check:** `store_copy_disk_check` at the start of the `store` suite.
- On x86 and aarch64 alike, QEMU's PCI order gave the store `virtio_blk` and its copy `virtio_blk#1`.
- After the reboot the copy is accepted as `BLANK` or `THE STORE'S`. Tighten that to `THE STORE'S` when your side writes it.
