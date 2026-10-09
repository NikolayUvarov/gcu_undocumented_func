# 251-STO-0010 — Speech models in the block store

**Type:** service (storage) · **Owner:** `STO` track · **Priority:** P3 · **Status:** open (decomposed 2026-10-09) · **Blocked by:** — (the store's own disk is done: [300-KRN-0025](../issues-done/300-KRN-0025-a-disk-for-the-block-store.done)) · **Main task:** [251](251-model-cache-and-model-disk.md) · **Roadmap:** track B · **Constitution:** MC-4.2, MC-4.5

Numbered from the tools track's request in `requests-STO.md` (2026-10-08, at the maintainer's request) by the storage session.

## Problem

The speech models of 250 and 252 come in the tools track's model cache and on a model disk. Each disk is a FAT32 volume labelled `MIND MODELS`, with `MANIFEST.json` and `<id>/<path>` files of 25 MB to 3 GB, each with its SHA-256 ([251](251-model-cache-and-model-disk.md)).

The maintainer wants them kept in MIND's own block store as well, so that a model is named, pinned and found by content like any other object (MC-4.2).

## Plan

- A way to import a model disk: each model becomes an object (its files as a DAG), published under a name per model id (for example `models/asr-ru-gigaam-v3-rnnt`) and pinned (303).
- The manifest's SHA-256 of each file is checked against the bytes before they are stored. Each object keeps the manifest entry with it, licence and terms included.
- Reading a model by name returns its files.
  - The recognizers and synthesizers of 250 and 252 map a large file read-only, so either the store hands out a memory object of a file, or a reader copies the file into one once.
  - Which of the two is decided when the work starts, with the durable disk in place.

## Decomposition (the storage session, 2026-10-09)

What stands in the way today, read from the code:
- **The index holds 4096 blocks** (`CAPACITY` in `blockstore/src/main.rs`, static memory) and is a sorted array: each insert shifts the entries after it. A 3 GB model is about 197 000 blocks of 16 KiB. At that size an insert moves megabytes, and the index alone, at 64 bytes an entry, takes more memory than a task's default quota of 16 MiB.
- **Reading `models:`** was 0.2 MB/s. The tools track's 251-APP-0023 makes it about 4 MB/s under QEMU; it is on the tools branch.
- **Objects are already written as a stream:** `dag::Builder` keeps one chunk and one node per level, so neither the writer nor the store holds an object whole.

| Task | What | State |
|---|---|---|
| [251-STO-0013](251-STO-0013-an-index-that-grows-with-the-medium.md) | An index that grows with the medium: a hash table with open addressing in place of the sorted array (O(1) insert and lookup), entries of 56 bytes, and its size chosen by the service from the medium at mount; a store of more blocks than it holds is still refused whole | built, host-tested |
| 251-STO-0014 | Importing a model disk: each file of `models:/MANIFEST.json` streamed into the store with its SHA-256 checked as it is read; each model an object of its files and its manifest entry, named `models/<id>` and pinned | after 0013 |
| 251-STO-0015 | A model read back by name: its files to a file system, or copied once into a memory object a recognizer maps (decided with 0014's measurements) | after 0014 |
| `KRN` request | `init` gives `blockstore` a memory quota that fits the index of its disk (today the default 16 MiB) | with 0013 |

The criterion "a model of 3 GB fits without the store holding it all in memory" is shown on the host. A host test stores a 3 GiB object through the store on a file-backed medium. Run locally, it records the index's size and the process's peak memory. QEMU without KVM reads about 4 MB/s, so 3 GB is not a CI case.

## Acceptance criteria

- On QEMU, after a model disk is imported, a model is read back by name. Its files match the manifest's SHA-256, and a restarted store finds it again.
- A model of 3 GB fits without the store holding it all in memory.

## Related

[251](251-model-cache-and-model-disk.md), [251-APP-0010](../issues-done/251-APP-0010-models-volume.done), [303](../issues-done/303-retention-and-collection.done), [docs/storage](../docs/storage/README.md).
