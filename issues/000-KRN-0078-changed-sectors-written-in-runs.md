# 000-KRN-0078 — Changed sectors written in runs, not one by one

**Type:** kernel (`vfs_server`) · **Owner:** kernel session · **Priority:** P1 · **Status:** in progress (`ram:`'s cost found and fixed on `claude/youthful-mendel-mf1soy`; landing and the MacBook Pro left) · **Blocked by:** — · **Roadmap:** track B · **Constitution:** MC-12.3

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

**2026-10-24, `ram:`'s cost found** (on `claude/youthful-mendel-mf1soy`, 2454e062 and 4464d659).
- **The cause: each write walked the file's whole FAT chain, twice** (`grows` and `write_raw`).
  - `format` gives a RAM disk up to 32 MiB clusters of 512 bytes, so `bench`'s 2 MiB make a chain of 4096 clusters.
  - `bench` writes in pieces of 16 KiB, so the chain was walked twice for each of 128 pieces.
  - The host test counted 532 862 sector reads to write the file. The stick's larger clusters made its chain short, which is why it was faster.
- **The first fix: the chain is walked once.**
  - The volume keeps the tail of the chain written last: its first cluster, its length and its last cluster.
  - It finds a cluster by walking on from the cursor, and appending keeps the cursor. Any other change to the FAT drops both, as it dropped the cursor.
  - Now 16 765 reads for the same file, about four a cluster.
- **The second fix: file data written in runs.**
  - Whole sectors of file data go past the cache in runs of neighbouring clusters, up to `BLOCK_MAX_SECTORS` a request, as reads already did (`Sectors::write_run`; in `disk.rs` a cached copy of each sector written is dropped once its request succeeds).
  - The volume is marked dirty first, as for any change. `docs/profile/threat-model.md` says what a power loss can leave now.
- **Measured** (`bench --quick`, x86, QEMU TCG in this session's container; emulation figures, not a machine's):

  | `ram:` | Writes | Reads |
  |---|---|---|
  | before (main 6bf90421) | 898 KiB/s | 16.7 MiB/s |
  | the chain walked once | 2.12 MiB/s | 16.8 MiB/s |
  | and the data in runs | 5.28 MiB/s | 16.8 MiB/s |

- **Checked:**
  - the x86 `vfs`, `block`, `disk`, `store` and `bench` suites: rc=0;
  - host tests `fat` (18, with `a_file_written_in_pieces_walks_its_chain_once`: the reads bounded, one write run a piece, the bytes as a model, the volume clean) and `vfs_zone`.
- **The gate:** passed in round 28 (`main` d5fd42db), every group but i9's, which is offline.
- **Left:** `bench files` on the MacBook Pro.

## Related

[176-KRN-0064](../issues-done/176-KRN-0064-bench.done) (the measurement), [211-KRN-0019](../issues-done/211-KRN-0019-boot-logs-on-the-log-partition.done), [000-KRN-0079](../issues-done/000-KRN-0079-a-returning-drive-is-checked-to-be-the-same-medium.done).
