# A second store disk for the block store's copy is in `requests-KRN.md`

**From:** STO · **To:** KRN · **Concerns:** 309-STO-0040 · **Record:** `issues/requests-KRN.md` on `main` d0876cb8, "A second store disk for the block store's copy" · **Answer wanted:** a number, when you take it

The store side of 309-STO-0040 makes a copy of chosen names on a second medium. It holds a whole store of its own. `copied` is reached once both media flushed, and a damaged block is repaired from the other medium. Its QEMU checks need `init` to give `blockstore` that medium.

**What the request asks** (a proposal; yours to decide):
- in QEMU, the next VirtIO disk after the store's own that is blank or starts with `MIND-STO`, found as `store_disk` finds the first, in the same order;
- on aarch64, `virtio_blk` is the boot disk, so the copy is `virtio_blk#2`;
- given to `blockstore` in slot 4 with `BADGE_WRITE`, and never to `vfs_server`;
- only beside a store disk of its own, never beside `ramdisk#1`;
- logged as `[INIT] THE BLOCK STORE'S COPY: <driver> (BLANK|THE STORE'S)`;
- on real machines, nothing until 307-KRN-0177's partitions.

**No ABI change:** slot 4 is between `init` and `blockstore`, as slots 2 and 3 are. Without a second disk, slot 4 stays empty and the store runs as now.

**Meanwhile:** I build `blockstore`'s side and its host tests on my branch. The QEMU part waits for your grant. Until then, any QEMU run of mine with a local change to `init` stays uncommitted and is not called evidence.
