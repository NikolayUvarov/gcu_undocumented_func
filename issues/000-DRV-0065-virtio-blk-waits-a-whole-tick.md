# 000-DRV-0065 — `virtio_blk`: a request waits a whole tick

**Type:** driver · **Owner:** `DRV` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — (found by [351-NET-0034](../issues-done/351-NET-0034-the-download-check-flushed-bounded-and-logged.done)) · **Constitution:** MC-5.6, MC-6.1

Numbered from the network track's request of 2026-10-20 in `requests-DRV.md`.

## Problem

- **What was measured.** On aarch64 (`virtio_blk` boot disk, QEMU TCG on un-rizen), `download` read an 8 MiB file back in 512 requests of 16 KiB. It took 5,080 ms, nearly all of it in the file service's reads, about 10 ms each.
- **Why.** `virtio_blk/src/main.rs`'s `wait` spins 1,000 times, then calls `mind::time::sleep(1)`. On aarch64 the tick is 10 ms, so every request that QEMU has not finished within the spins costs a whole tick. Under TCG that is nearly every one.
- **What it costs.** The file service reads at about 1.6 MB/s on aarch64. On x86 (1 ms tick) the same check reads 30 MiB in about 5 s.
- **The same pattern** is in `ahci`'s `wait` ([000-DRV-0064](000-DRV-0064-ata-and-ahci-waits-with-a-deadline.md)).

## Plan

1. **Wait on the queue's interrupt,** or poll the used ring against `monotonic_ns`, with a short sleep only once a request is known to be slow.
2. **Give each request a deadline in time,** as 000-DRV-0064 does for `ata`.

## Acceptance criteria

- On aarch64, the `net` suite's `TIME: download's … check` lines show reads well under a tick each.
- The `vfs` and `store` suites pass.

No real machine is needed.

## Related

[351-NET-0034](../issues-done/351-NET-0034-the-download-check-flushed-bounded-and-logged.done), [000-DRV-0064](000-DRV-0064-ata-and-ahci-waits-with-a-deadline.md).
