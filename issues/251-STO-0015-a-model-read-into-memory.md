# 251-STO-0015 — A model read back into memory

**Type:** service (storage) · **Owner:** `STO` track · **Priority:** P3 · **Status:** open · **Blocked by:** — · **Main task:** [251](251-model-cache-and-model-disk.md) · **Roadmap:** track B · **Constitution:** MC-4.2, MC-4.5

Split from [251-STO-0010](../issues-done/251-STO-0010-speech-models-in-the-store.done) by the storage session (2026-10-10), whose criteria were met without it.

## Problem

A model in the block store is read back today by `blocks models get`, which copies one file to a file system, checked against its SHA-256 ([251-STO-0014](../issues-done/251-STO-0014-importing-a-model-disk.done)). The recognizers and synthesizers of [250](250-voice-dictation.md) and [252](252-neural-speech-synthesis.md) map a model's large file read-only. A file of up to 3 GB copied to a volume first doubles the space it takes and the time to start.

## Plan

- **Decide between two ways**, by measurement on the platform:
  - the store hands out a memory object holding one file of a model object;
  - a reader copies the file once, by the model's name, into a memory object that the recognizer maps.
- **The interface:** whichever is chosen gets a new version of `idl/blockstore.wit` or of the reader's interface, with an explicit transition (MC-12.4).
- **What the platform needs first:**
  - the store's memory quota for a large disk's index (requests-KRN.md);
  - a model disk read faster than about 4 MB/s under QEMU.

## Acceptance criteria

On QEMU, a program standing in for a recognizer maps a model's file from the store by the model's name, and its bytes match the manifest's SHA-256.

## Related

[251-STO-0010](../issues-done/251-STO-0010-speech-models-in-the-store.done), [251-STO-0014](../issues-done/251-STO-0014-importing-a-model-disk.done), [docs/storage](../docs/storage/README.md).
