# 301 — Objects larger than a block: manifests as a Merkle-DAG (track B, second step)

**Type:** main task · **Owner:** `STO` track · **Priority:** P2 · **Status:** in progress · **Blocked by:** — (the service parts wait for [300-KRN-0001](../issues-done/300-KRN-0001-blockstore-at-boot.done) (done), like 300) · **Roadmap:** track B "manifests/Merkle-DAG" · **Constitution:** MC-4.2, MC-4.7, MC-4.13, Appendix B.3

## Problem

The block store holds blocks of at most 16 KiB ([300](300-checksummed-block-store.md)). MC-4.2 asks that published objects and structural manifests are immutable and verifiable by content. Appendix B.3 continues the path from immutable blocks to manifests and a Merkle-DAG: an object of any size is named by one root CID, and every part of it can be verified on the way from the root.

## Plan

The `STO` track numbers its tasks itself and may change the split.

- `301-STO-0001` — the format and a library (`mind::dag`).
  - Fixed chunks of 16 KiB as `raw` blocks, and DAG-CBOR nodes `{"v": 1, "size": n, "links": [...]}` of up to 256 links.
  - The tree's shape follows from the size alone, so the same bytes always give the same root.
  - A streaming builder and a reader that checks every node and chunk against its CID and its place in the shape.
  - Host tests against an independent reference.
- `301-STO-0002` — the block store takes nodes. `put` names the content type, and a `dag-cbor` block must decode as a node of this schema before it is stored (MC-4.2: the type bound to the data).
- `301-STO-0003` — objects over the service.
  - `mind::dag::Blocks` for a `blockstore` client.
  - A tool that puts a file as an object and reads it back, with its QEMU check. This waits for the service to run (300-STO-0003).
- **Later main tasks:** names and roots (Head/Refs, MC-4.3), retention and garbage collection by reachability (MC-4.5, 4.11).

## Acceptance criteria

- The roots and node bytes equal those of an independent implementation for sizes around every boundary of the shape.
- Non-canonical nodes, trees out of shape and blocks that do not match their CID are refused, each with a test.
- An object is put and read back through the running service in QEMU, and a damaged chunk of it is refused there.

## Progress (2026-10-06)

- **Done — [`301-STO-0001`](../issues-done/301-STO-0001-object-format.done):** `mind::dag` and `tests/dag_host.rs`.
- **Done — [`301-STO-0002`](../issues-done/301-STO-0002-store-takes-nodes.done):** `put(codec, data)`; a node is stored only if it decodes, and a record typed as a node that does not is corrupt.
- **Next — `301-STO-0003`:** objects over the running service. It waits, like 300-STO-0003, for [300-KRN-0001](../issues-done/300-KRN-0001-blockstore-at-boot.done) (done).

## Related

[docs/storage](../docs/storage/README.md); [300](300-checksummed-block-store.md); Constitution Article 4, Appendix B.3.
