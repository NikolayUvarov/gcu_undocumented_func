# 028 — Memory rights, read-only sharing and leases (roadmap C4, part 1)

**Type:** architecture · **Priority:** P1 · **Status:** open · **Roadmap:** step 3, C4 · **Constitution:** MC-2.6, MC-2.7, MC-3.6

## Problem

A memory capability always maps read-write, so every share is `SHARE_RW`. Revoking a memory capability (C2) leaves the mappings already made from it, so a lease cannot be ended.

## Plan

- Memory capabilities carry rights: `CAP_READ` (map), `CAP_WRITE` (map writable). `MEM_SHARE` returns both; `CAP_MINT` narrows them like endpoint rights. `MEM_MAP` maps read-only without `CAP_WRITE`.
- Every foreign mapping records the derivation node of the capability it was made from. `CAP_REVOKE` also unmaps every mapping made from a removed capability (**LEASE**: copy, use, revoke).
- Completion point: if an affected task is running on another CPU, the revoking task waits until that CPU has switched address space (TLB flushed); when `CAP_REVOKE` returns no removed mapping is usable anywhere.
- `MEM_UNMAP` accounting and the shared-mapping quota are unchanged.

## Acceptance criteria

- Isolation fixture: a read-only mint maps read-only (a write faults in a child task) and cannot be widened; revoking a lease unmaps it in the holder (an access afterwards faults).
- All QEMU suites pass.

## Related

[026](../issues-done/026-derivation-and-revocation.done), [029](029-memory-objects-move-seal.md), [ROADMAP](../ROADMAP.md) C4.
