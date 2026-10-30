# 309-STO-0031 — A copy of named roots in a file: export, import, and repair from it

**Type:** storage (`libmind` `car`, `blocks`) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [309](309-copies-scrub-and-repair.md) · **Constitution:** MC-4.4, MC-4.8, MC-6.8

Split from [309](309-copies-scrub-and-repair.md) (2026-10-30), its second step, after the scrub. Unlike 309's live second store, it needs no second medium lent to the store, so it waits for no other track.

## Problem

The store keeps one copy on one medium. A damaged block is found by a read or a scrub ([309-STO-0025](../issues-done/309-STO-0025-scrub.done)), but there is nothing to repair it from. A lost medium, or a RAM disk after a reset, loses every name. And an instance cannot be moved to another machine ([313](313-persistent-actors.md)), as nothing carries its blocks and names there.

## Plan

- **The format is CAR v1** (the IPLD content archive), so a copy is readable by other tools:
  - **a header** (a varint length, then DAG-CBOR `{"roots": [cid], "version": 1}`). Its one root is a names node;
  - **sections**, each a varint length, then a CID, then its block.
- **The names node:** canonical DAG-CBOR `{"v": 1, "names": [[name, version, root], ...]}`, sorted by name, each name's root linked. It is decoded only if it is exactly that encoding, as `tree` does.
- **`mind::car` (`libmind/src/car.rs`):** no heap and no system calls.
  - A writer of sections and a reader of them.
  - Every block read is checked against its CID. A section cut short, a length past a block, a CID of another hash or codec, and a header that is not exactly its encoding are refused.
- **`blocks export <file> <name>...`:**
  - the names' current roots, read at one point (`snapshot`);
  - the names node, then every block under each root (`tree::walk`). A block met twice may be written twice; an import puts it once;
  - it prints the number of blocks and bytes, and the SHA-256 of the file.
- **`blocks import <file>`:**
  - checks every section first, and refuses the whole file on one bad section, with nothing put;
  - then puts each block. A block the store holds damaged is repaired by the put, as a put of the same bytes repairs it now;
  - then publishes each name at the file's root by compare-and-swap, from the version it is at in this store (0 for a new name). It prints each name published, or a name whose current root differs, which it leaves as it is: a conflict, never overwritten.
- **`blocks import --check <file>`** reads and checks a file, and puts nothing.
- **Independence stated:** a file on the store's own medium is not an independent copy; docs/storage says what is.

## Acceptance criteria

- Host tests (`tests/car_host.rs`, through the store's logic on a simulated medium):
  - CAR v1 vectors (a header and a section built by hand); every truncation, a byte changed in any section, a CID of another hash or codec, and a non-canonical names node refused;
  - export then import into an empty store gives the same names, roots and blocks;
  - an import into a store holding a damaged block repairs it, and a scrub then finds nothing;
  - a name whose root differs is left as it is and reported; nothing is put from a file with one bad section.
- QEMU `storefaults` (x86, aarch64): a chunk damaged on the medium is found by `blocks scrub`, then repaired by `blocks import` from a copy exported before, and the next scrub finds nothing.
- QEMU `store` (x86, aarch64): names exported to a file on the boot volume come back, whole, into a store that lost them (a RAM disk after a reset, or a blank store disk).
- docs/storage and the profile.

## Related

[309](309-copies-scrub-and-repair.md), [309-STO-0025](../issues-done/309-STO-0025-scrub.done), [311-STO-0027](311-STO-0027-directories-as-objects.md), [313](313-persistent-actors.md) (a move through a copy of its store).
