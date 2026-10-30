# 311-STO-0027 — Directories as objects: a directory node, its walk, and the store's reachability

**Type:** storage (`libmind` `tree`, `blockstore`) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [311](311-storefs-a-file-face.md) · **Constitution:** MC-4.2, MC-4.5, MC-4.13

Split from [311](311-storefs-a-file-face.md) (2026-10-30), its first step at the maintainer's word: "trees as objects" needs no partition of the store's own, so it does not wait for 307.

## Problem

An object in the store is one file's bytes ([301](../issues-done/301-objects-as-merkle-dags.done)). There is no directory: a set of named files and subdirectories with one root. The store's checks and its retention know only file trees. A directory node would be refused at `put` as a `dag-cbor` block of no known schema. A file reached only through a directory would be collected.

## Plan

- **`mind::tree` (`libmind/src/tree.rs`)**, with no heap and no system calls:
  - **A directory node** is one canonical DAG-CBOR block, `{"v": 1, "entries": [[name, kind, size, link], ...]}`:
    - keys in DAG-CBOR's order: `v`, then `entries`;
    - `name`: 1 to 255 bytes of UTF-8, without `/` or NUL, and neither `.` nor `..`;
    - `kind`: 0 a file, 1 a directory;
    - `size`: a file's bytes, or a directory's entry count;
    - `link`: the CID as `dag` writes its links. A directory's link is `dag-cbor`; a file's is `raw` or `dag-cbor`;
    - the entries are sorted by the names' bytes, each name once;
    - an empty directory is `{"v": 1, "entries": []}`;
    - at most one block (`dag::CHUNK` bytes) for now. A larger one is refused, and sharding is a later task.
  - **It decodes only if it is exactly that encoding.** What was read is encoded again and compared, as `dag::decode` does.
  - **The walk** visits every block under a root, a file tree's or a directory's:
    - a file is walked with `dag::walk`;
    - a directory, with an explicit stack of (CID, next entry) of a bounded depth. A parent node is read again from its CID rather than kept, so one `CHUNK` buffer serves the walk;
    - each node is checked against its CID and its schema, and each entry's size against what its object holds;
    - it returns the bytes of the files under the root.
- **`blockstore`:**
  - `put` takes a `dag-cbor` block that is a file node or a directory node, and refuses anything else (`invalid`);
  - the store's walk (publication and pin completeness, quotas, collection) uses the tree's walk, so a directory's files are retained by the name that holds it.
- **Not here:** the working tree in memory, the publish by compare-and-swap of a changed tree, `storefs` (a `vfs.wit` provider), the shell's commands. These come in the next tasks of 311.

## Acceptance criteria

- Host tests (`tests/tree_host.rs`):
  - a directory encodes and decodes back to the same entries;
  - only the canonical encoding decodes: every changed byte of a valid node is refused or reads as what it encodes;
  - names unsorted or twice, bad names, kinds and links, and a node over a block are refused;
  - the walk of a nested tree visits exactly its blocks;
  - a missing block is `not-found`, a changed one `corrupt`, a tree past the depth `shape`.
- Host tests of the store (`tests/blockstore_host.rs`):
  - a published directory tree keeps its files across a collection, and an unretained one is freed;
  - a publication of a tree missing a file's block is `incomplete`;
  - a directory node is stored, and a `dag-cbor` block of neither schema refused.

## Related

[311](311-storefs-a-file-face.md), [301](../issues-done/301-objects-as-merkle-dags.done), [303](../issues-done/303-retention-and-collection.done), [docs/storage](../docs/storage/README.md).
