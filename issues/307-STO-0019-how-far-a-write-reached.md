# 307-STO-0019 — How far a write reached: durability classes in `blockstore.wit` 2.0

**Type:** service and interface (storage) · **Owner:** `STO` track · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [307](307-the-stores-own-partition.md) · **Constitution:** MC-4.4, MC-12.1, MC-12.3, MC-12.4

## Problem

Every write of the store is followed by its medium's flush, but nothing tells a client what that reached. On a real machine the store runs on a RAM disk, where nothing outlives a reset, and a client cannot tell that from a disk (MC-4.4, MC-12.3).

## Plan

- **`blockstore.wit` 2.0** adds `enum durability { volatile, flushed, copied }`:
  - `volatile`: the store's medium is a RAM disk (`KIND_RAM`);
  - `flushed`: the medium's driver acknowledged a flush after the write. What a flush reaches is the driver's to state (`requests-DRV.md`, for 307);
  - `copied`: two media flushed ([309](309-copies-scrub-and-repair.md)), not reached yet.
- **Who reports it:**
  - `publish`, `unpublish`, `commit` and `pin` reply with it (`published`, `committed`, `pin-taken`);
  - `stat` gives the medium's class;
  - a `put` stays as it was: until a name or a pin retains its object, a block is retained only by its lease (MC-4.11), so its class is the medium's, in `stat`.
- **1.x is refused** by the wire's major version, with no conversion code (AGENTS.md section 3).
- **Clients:** `blocks`, its `models` part and `tally` print or use the new replies. `fm` (APP's) builds unchanged: it drops the replies of publish, unpublish and commit.

## Acceptance criteria

- The `store` suite: the RAM disk's store says `DURABILITY=VOLATILE` in its log and `stat`, and a publication reports `(VOLATILE)`.
- The store-disk check (`boot` suite, 300-KRN-0025): on a VirtIO disk, `stat` says `FLUSHED`, and a publication reports `(FLUSHED)` and is found after a reboot.
- Both on x86 and aarch64. The host tests and `tests/idl_fuzz_host.rs` (the new records) pass.

## Related

[307](307-the-stores-own-partition.md), [309](309-copies-scrub-and-repair.md), `idl/blockstore.wit`, docs/storage.
