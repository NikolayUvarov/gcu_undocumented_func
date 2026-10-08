# Requests for the storage track (STO), not numbered yet

**Owner:** storage track · **Status:** open · **Recorded by:** the tools track (APP), 2026-10-08, for main task [251](251-model-cache-and-model-disk.md) at the maintainer's request

The storage track numbers its own tasks (`NNN-STO-MMMM`), so requests from other tracks wait here. The storage track turns each into a task and removes it from this file, and the file goes when it is empty.

## Speech models in the block store

### Problem

The speech models of 250 and 252 come in the tools track's model cache and on a model disk. Each disk is a FAT32 volume labelled `MIND MODELS`, with `MANIFEST.json` and `<id>/<path>` files of 25 MB to 3 GB, each with its SHA-256 ([251](251-model-cache-and-model-disk.md)).

The maintainer wants them kept in MIND's own block store as well, so that a model is named, pinned and found by content like any other object (MC-4.2).

### Plan (a proposal; the storage track decides)

- A way to import a model disk: each model becomes an object (its files as a DAG), published under a name per model id (for example `models/asr-ru-gigaam-v3-rnnt`) and pinned (303).
- The manifest's SHA-256 of each file is checked against the bytes before they are stored. Each object keeps the manifest entry with it, licence and terms included.
- Reading a model by name returns its files.
  - The recognizers and synthesizers of 250 and 252 map a large file read-only, so either the store hands out a memory object of a file, or a reader copies the file into one once.
  - Which of the two is the storage track's choice.

### Acceptance criteria

- On QEMU, after a model disk is imported, a model is read back by name. Its files match the manifest's SHA-256, and a restarted store finds it again.
- A model of 3 GB fits without the store holding it all in memory.
