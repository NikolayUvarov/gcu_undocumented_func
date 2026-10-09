# 251-STO-0010 — Speech models in the block store

**Type:** service (storage) · **Owner:** `STO` track · **Priority:** P3 · **Status:** open · **Blocked by:** [requests-KRN.md](requests-KRN.md) ("A durable disk for the block store": the store runs on an 8 MiB RAM disk, so a model of 25 MB to 3 GB does not fit and nothing survives a restart of the machine) · **Main task:** [251](251-model-cache-and-model-disk.md) · **Roadmap:** track B · **Constitution:** MC-4.2, MC-4.5

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

## Acceptance criteria

- On QEMU, after a model disk is imported, a model is read back by name. Its files match the manifest's SHA-256, and a restarted store finds it again.
- A model of 3 GB fits without the store holding it all in memory.

## Related

[251](251-model-cache-and-model-disk.md), [251-APP-0010](../issues-done/251-APP-0010-models-volume.done), [303](../issues-done/303-retention-and-collection.done), [docs/storage](../docs/storage/README.md).
