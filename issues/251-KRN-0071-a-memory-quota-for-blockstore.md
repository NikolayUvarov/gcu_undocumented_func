# 251-KRN-0071 — A memory quota for `blockstore` that fits its index

**Type:** kernel (`init`'s policy) · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress (made and tested in QEMU; the gate left) · **Blocked by:** — · **Roadmap:** track B · **Constitution:** MC-6.5

## Problem

The storage session's request, for [251-STO-0013](../issues-done/251-STO-0013-an-index-that-grows-with-the-medium.done):

- **How the index is sized.** The block store's index grows with its medium: 56 bytes a slot, up to 2^20 slots (`slots_for` in `blockstore/src/store.rs`).
- **How it is cut.** `blockstore` halves the index until its memory quota allows it.
- **The quota.** It was the default 16 MiB, which keeps the index under about 14 MiB: roughly 230 000 blocks of 16 KiB, or 3.5 GiB of objects.
- **What breaks.** A larger store disk mounted with an index too small for its blocks, and mounting refuses such a store whole.

## Plan

`init` gives `blockstore` a quota of 64 MiB (`BLOCKSTORE_MEMORY_MIB`), next to `windows` and `compositor`. That holds the largest index (2^20 slots, 56 MiB) and what the store held before. A quota is a limit, not a reservation: a small disk's store takes no more than before.

## Acceptance criteria

1. On a store disk of 8 GiB, `[BLOCKSTORE] INDEX:` reports the 2^20 slots `slots_for` asks for (57 344 KiB), not a halved number.
2. The check runs in QEMU: `store_disk_check` boots with a sparse 8 GiB store disk.

## Progress

**2026-10-10.** Done in `init/src/main.rs`. The boot suite passes on x86 with the new check: `[BLOCKSTORE] INDEX: 1048576 SLOTS (57344 KiB) FOR 16777216 SECTORS`.

## Related

[251-STO-0013](../issues-done/251-STO-0013-an-index-that-grows-with-the-medium.done), [300-KRN-0025](../issues-done/300-KRN-0025-a-disk-for-the-block-store.done).
