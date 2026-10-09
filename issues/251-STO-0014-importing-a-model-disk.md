# 251-STO-0014 — Importing a model disk into the block store

**Type:** service (storage) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [251-STO-0010](251-STO-0010-speech-models-in-the-store.md) (in [251](251-model-cache-and-model-disk.md)) · **Roadmap:** track B · **Constitution:** MC-4.2, MC-4.5, MC-11.11

## Problem

A model disk (`models:`) holds speech models as files, with `MANIFEST.json` giving each file's size and SHA-256 and each model's licence and terms. The maintainer wants them in MIND's own store too, so that a model is named and found by content (MC-4.2).

## Plan

- **The JSON:** `mind::json`, a reader without allocation, and `mind::models`, the manifest as the importer needs it.
  - The parser service reads it (`idl/parse.wit` 1.2, `model`). `blocks` holds the store's write client and parses none of it (MC-11.11).
- **`blocks models import [id]`:** one object per model, written as a stream through `dag::Builder`.
  - A text header comes first: the model's id, the manifest's entry whole, and a line per file.
  - Then the files' bytes, each checked against its size and SHA-256 as it is read.
  - The object is named `models/<id>`, so the name retains every file. A model with a file that differs is not named.
- **`blocks models get <id> <path> <file>`:** one file copied out by the model's name, checked again.
- **Tests:**
  - `tests/models_host.rs`: JSON against Python, and the manifest of `models/manifest.toml`;
  - the `disks` check: import, get, the store restarted, a bad model not named.

## Acceptance criteria

On QEMU (x86 and aarch64), after a model disk is imported, a model is read back by name, its files match the manifest's SHA-256, and a restarted store finds it again (251-STO-0010's first criterion).

## Related

[251-STO-0010](251-STO-0010-speech-models-in-the-store.md), [251-STO-0013](251-STO-0013-an-index-that-grows-with-the-medium.md), [351-NET-0011](../issues-done/351-NET-0011-parse-release-metadata.done) (the parser's other formats).
