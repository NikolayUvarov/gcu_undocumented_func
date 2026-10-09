# 251-STO-0013 — An index that grows with the medium

**Type:** service (storage) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open (built and host-tested; QEMU store suites and the 3 GiB run to come) · **Blocked by:** — · **Main task:** [251-STO-0010](251-STO-0010-speech-models-in-the-store.md) (in [251](251-model-cache-and-model-disk.md)) · **Roadmap:** track B · **Constitution:** MC-4.2, MC-4.5

## Problem

The block store's index was a sorted array of 4096 entries in static memory: an insert shifted every entry after it. A 3 GB model is about 197 000 blocks of 16 KiB. At that size each insert would move megabytes, and 64-byte entries would take more memory than a task's default quota of 16 MiB.

## Plan

- **The index:** a hash table with open addressing and linear probing (`blockstore/src/store.rs`).
  - A slot is 56 bytes: the lease's start, the record's sector (32 bits), the length, the used and live flags, the CID's binary form.
  - A removal moves back the entries of its run that would no longer be found (no tombstones).
  - A small index (up to 64 slots) takes every slot; a large one takes seven eighths, so a search for a missing block ends soon.
  - Mount empties the slots first.
- **Its size:** `slots_for(sectors)` gives room for a block per 8 sectors, at least 4096 blocks (so the RAM disk's store keeps its capacity of 4096), at most 2^20 slots. The service allocates them at mount, halving until its memory quota allows, and logs `[BLOCKSTORE] INDEX:`.
- **A medium of more than 2^32 sectors** is refused with `too-large`.
- **A larger quota for `blockstore`** is `init`'s: requested in [requests-KRN.md](requests-KRN.md).

## Acceptance criteria

- Host tests: 100 000 blocks put, found after a remount, half collected and the rest found; removals keep every other block found over many rounds in a small index; the store's other tests unchanged.
- QEMU (`store` and `storefaults`, x86 and aarch64): as before, with the index's line.
- A 3 GiB object stored on a file-backed medium on the host, found after a remount, with the index's size and the process's peak memory recorded (251-STO-0010's criterion, run locally).

## Related

[251-STO-0010](251-STO-0010-speech-models-in-the-store.md), [300-STO-0002](../issues-done/300-STO-0002-blockstore-service.done).
