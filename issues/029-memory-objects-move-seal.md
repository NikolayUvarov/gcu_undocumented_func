# 029 — Memory objects: MOVE and sealed SHARE_RO (roadmap C4, part 2)

**Type:** architecture · **Priority:** P1 · **Status:** open · **Roadmap:** step 3, C4 · **Constitution:** MC-2.6, MC-2.7, MC-2.8

## Problem

Shared memory always stays a heap block of its creator, who keeps a writable mapping. There is no transfer with a single owner (MOVE) and no way for a receiver to know that nobody can write the memory while it reads it (SHARE_RO).

## Plan

- `MEM_DETACH(address)`: takes a heap block out of the caller's address space and returns a capability that is the only reference to it (a memory object). The memory is freed when no capability or mapping refers to it.
- Copying a memory capability (IPC transfer without `CAP_TRANSFER_MOVE`, a grant without `GRANT_MOVE`, `CAP_MINT`) requires `CAP_GRANT`; a detached object has no `CAP_GRANT`, so it can only be moved: **MOVE** has a single commit point (the transfer) and never two owners.
- `CAP_INFO` reports whether a memory range is **sealed**: no capability with `CAP_WRITE`, no writable mapping and no DMA region overlaps it. A receiver that sees a sealed read-only capability has `SHARE_RO`.

## Acceptance criteria

- Isolation fixture: a detached object cannot be copied, only moved; after detaching, the old address faults; a read-only object whose writable capability was dropped is reported sealed, otherwise not.
- All QEMU suites pass.

## Related

[028](028-memory-rights-and-lease.md), [ROADMAP](../ROADMAP.md) C4.
