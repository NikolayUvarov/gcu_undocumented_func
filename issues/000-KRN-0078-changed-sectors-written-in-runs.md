# 000-KRN-0078 — Changed sectors written in runs, not one by one

**Type:** kernel (`vfs_server`) · **Owner:** kernel session · **Priority:** P1 · **Status:** in progress (made; the MacBook Pro measured; `ram:` still slow) · **Blocked by:** — · **Roadmap:** track B · **Constitution:** MC-12.3

## Problem

`bench` on the MacBook Pro (2026-10-11) measured these rates:

| Volume | Writes | Reads |
|---|---|---|
| `ram:` (a RAM disk) | 1.57 MiB/s | 46.8 MiB/s |
| `data/` (the USB stick) | 1.75 MiB/s | 32.4 MiB/s |
| `log:` (the USB stick) | 1.60 MiB/s | 32.3 MiB/s |

In QEMU, `ram:` wrote at 420 KiB/s. The medium made no difference, so the cost is in `vfs_server`.

`vfs_server/src/disk.rs` caches 64 sectors with write-back:

- A `flush` writes the changed ones in runs of neighbours.
- But a changed line evicted to make room was written alone: one block request, an IPC round trip with a lent buffer, for each 512-byte sector.
- A file larger than the cache was therefore written a sector a request. 2 MiB took 4096 requests of about 300 µs, about 1.2 s, as measured.
- Reads come 32 sectors a request (read-ahead), which is why they were 30 times faster.

## Plan

Eviction writes as `flush` does. A changed line about to be evicted is written together with every other changed line, in LBA order, neighbours in one request. Nothing else changes:

- A run that fails stays dirty.
- The volume's dirty mark in FAT[1] still covers a power loss.

## Acceptance criteria

- **QEMU.** `bench files` writes `ram:` several times faster than before. The `vfs`, `block`, `disk` and `bench` suites pass.
- **The MacBook Pro.** `bench files` is recorded again.

## Progress

**2026-10-11, the MacBook Pro** (build 8223d24, `bench0001.txt`), writing 2 MiB:

| Volume | Before | Now | Small files a second |
|---|---|---|---|
| `ram:` | 1.57 MiB/s | 2.32 MiB/s (860 ms) | 412 |
| `data/` (the USB stick) | 1.75 MiB/s | 5.65 MiB/s (353 ms) | 72 |
| `log:` (the USB stick) | 1.60 MiB/s | 4.42 MiB/s (451 ms) | 82 |

Reads are as before (39–54 MiB/s). Writes to the USB stick are three times faster, but `ram:` gained only half. Its cost is elsewhere than the requests per sector, and it is still to be found: `ram:` writes 2 MiB in 860 ms, `data/` on the stick in 353 ms.

## Related

[176-KRN-0064](../issues-done/176-KRN-0064-bench.done) (the measurement), [211-KRN-0019](211-KRN-0019-boot-logs-on-the-log-partition.md), [000-KRN-0079](../issues-done/000-KRN-0079-a-returning-drive-is-checked-to-be-the-same-medium.done).
