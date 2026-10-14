# 300-STO-0016 — An object's tree walked once, and `blocks check` in one pass

**Type:** storage (library, tool, tests) · **Owner:** `STO` track · **Priority:** P2 · **Status:** in progress (built and host-tested; the aarch64 group on the slowest machine is left) · **Blocked by:** — · **Main task:** [300](../issues-done/300-checksummed-block-store.done) · **Roadmap:** track B · **Constitution:** MC-4.2, MC-5.2, MC-12.2

From the request "The store suite's `blocks check` of the 4 MiB object outruns its 240 s on a slower machine" in requests-STO.md (recorded by the tools track, 2026-10-14, from its gate of eda1c74).

## Problem

**What was seen.** On `DESKTOP-i7-930-wsl-u24` (aarch64 under TCG, nothing else running) the store suite's `blocks check <root> pattern` of the 4 MiB + 1 byte object ran past its 240 s. It passed on un-rizen-service, and once timed out on i9 at a load of 46.

**Why it was slow** (found reading the code, then measured on the host):
- **`dag::walk`, which `dag::complete` and the store's collection use, went down from the root once for every chunk, not once for every node of height 1.** Its step was `capacity(1)`, the bytes under one child of a node of height 1, which is one chunk, instead of `capacity(2)`, the bytes under a node of height 1. It then visited all the links of that node each time.
  - For an object of 260 chunks it made 65 813 visits instead of 263 (`tests/dag_host.rs`).
  - In `blocks check`, `complete` asks the store `has` for each, so that was about 65 000 calls.
  - Each time it also fetched the root and the node of height 1, and checked each against its CID: 257 times about 9 KiB through SHA-256.
- **`blocks check` then read the object with `read_at` one chunk at a time.** Each call fetched the root for the size, then the root, the node of height 1 and the chunk, each checked against its CID: four gets and about 25 KiB through SHA-256 for every 16 KiB.

## Plan

- **`dag::walk` steps by the bytes under a node of height 1**, as its comment always said. Each chunk and each node of height 1 is visited once, and the root once for each node of height 1 under it. This makes `dag::complete` and the store's marking cheaper too.
- **`dag::each_chunk`** reads an object in order, one chunk at a time, to a closure with the chunk's offset. Each node is fetched and checked once for the chunks under it, and each chunk in one get, against its CID and its place in the shape, as `read_at` checks them.
- **`blocks check`** uses it in one pass, without `complete` before it. A missing block or a corrupt one is reported as before, when it is reached. The first byte unlike the pattern stops it.
- **The suite** prints how long the 4 MiB put and checks took, by the host's clock. It waits 900 s for each, which the request proposes for the slowest machine listed for the group.

## Acceptance criteria

- **Host tests (`tests/dag_host.rs`):**
  - `walk` visits each block of the tree once;
  - `each_chunk` gives every chunk in order, for objects of 1 byte to height 2, with one get for each chunk and each node once;
  - it refuses a chunk that does not match its CID, and a missing one, as `read_at` does.
- The store and storefaults suites pass on x86 and aarch64, and print the check's time.
- The group "aarch64: files, network and TLS" passes on `DESKTOP-i7-930-wsl-u24` with nothing else running there.

## Related

[300-STO-0003](../issues-done/300-STO-0003-blocks-tool-and-store-suite.done) (`blocks` and the store suite), [303](../issues-done/303-retention-and-collection.done) (the collection's marking).
