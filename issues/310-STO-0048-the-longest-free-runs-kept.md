# 310-STO-0048 — The store keeps its longest free runs when it meets more than its list holds

**Type:** bug (storage) · **Owner:** `STO` track · **Priority:** P2 · **Status:** in progress (2026-11-12) · **Blocked by:** — · **Main task:** [310](310-the-index-on-the-medium.md) · **Roadmap:** track B "scale" · **Constitution:** MC-4.4, MC-4.10

## Problem

The store finds room for a record in a list of the medium's blank runs (`blockstore/src/store.rs`, `holes`): 512 on the platform, 64 in the host tests. A scan (a mount, a collection) adds the runs it meets in the medium's order and dropped every run past the list. A blank sector freed later was dropped too once the list was full.

So a medium with more small gaps than the list holds before its free tail lost the tail until the next scan, and the next scan lost it again:
- **How the gaps come:** a journal appended many times publishes its name once for each entry. A collection frees the superseded name records, one sector each, between the entries' blocks.
- **What it did:** every block of two sectors or more (any of more than about 450 bytes) was refused `full`, while most of the medium was free.
- **Found by** 312-STO-0047's host test: a build after a remount of a journal of 300 entries was refused.

## Plan

- When the list is full, its shortest run gives way to a longer one, for a scan's runs and for a sector freed later. The list keeps the longest runs met, in the medium's order; the shorter ones are found again by the next scan, as before.
- No change to the layout, the interface or what is written.

## Acceptance criteria

- **Host test:** a list of 8 runs, 20 gaps of one sector between blocks, then the rest of the medium. After the collection that makes the gaps, the list holds the rest of the medium. A block of many sectors is put, and after a mount the same holds.
- **The test fails without the change.**
- The store's other host tests pass.

## Related

[310](310-the-index-on-the-medium.md), [312-STO-0047](312-STO-0047-a-vector-index-of-a-journals-embeddings.md), [303](../issues-done/303-retention-and-collection.done).
