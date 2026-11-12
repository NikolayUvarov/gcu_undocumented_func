# 312-STO-0047 — A vector index of a journal's embeddings, an exact scan

**Type:** library (storage) · **Owner:** `STO` track · **Priority:** P2 · **Status:** in progress (2026-11-12) · **Blocked by:** — · **Main task:** [312](312-the-minds-memory.md) · **Roadmap:** track B "assertions and derived indexes" · **Constitution:** MC-4.5, MC-4.6, MC-4.12, MC-4.13, MC-10.3

## Problem

312's plan has two derived indexes: full text, done by [312-STO-0035](../issues-done/312-STO-0035-a-text-index-derived-from-a-journal.done), and vectors. Who does what was settled on 2026-10-27:
- `NRL`'s runtime computes the embeddings, and this track keeps the index;
- the index's manifest names the model's CID and the index's parameters, so it can be rebuilt and a stale one detected;
- an exact scan comes first, and an approximate index (HNSW) when sizes ask for it, its recall measured against the exact scan.

Nothing keeps embeddings or answers which objects are nearest to a query.

## Plan

1. **An embedding** (`libmind/src/vector.rs`):
   - one canonical DAG-CBOR map `{"v": 1, "model", "vector", "subject"}`, the components signed bytes;
   - the model's and the object's CIDs as byte strings, mentioned, not retained;
   - the content of a journal entry of kind `embedding`, as an assertion is (312-STO-0038).
2. **The index:**
   - a `tree` with a `manifest` (journal, head, model, dimensions, metric `dot`, count) and `vectors`;
   - `vectors` is files of the model's vectors with their sequences, newest first, each full but the last;
   - the same head gives the same root.
3. **`build`, `state`, `nearest`, `scan` and `verify`,** as the text index's:
   - a search scores every vector of the index and of the entries above its head;
   - it checks each answer against its entry (a false one refuses the index);
   - the exact scan answers without the index;
   - a check rebuilds and compares.
4. **The metric:** the dot product of signed bytes, exact. Of equal scores, the newest first.
5. **`blocks vector add|build|near|scan|check`:** the store runs no model, so `add` takes the vector its caller gives.

## Acceptance criteria

- **Host tests:**
  - the encodings are the schemas';
  - an index is built twice to one root, the root of the layout written in the test, also after a remount and a collection;
  - every search answers the exact scan and a model, also stale;
  - an index changed by hand is refused by a search (a vector raised or misplaced) or told by a check (one dropped);
  - refused: another journal, a query of another model or dimensions, and an embedding by the model of other dimensions;
  - faults put in by hand are caught.
- **QEMU `store` suite** (x86 and aarch64):
  - an embedding's CID and the index's root are the ones the suite computes;
  - searches answer as its scores order them, stale and rebuilt;
  - the refusals;
  - on aarch64, an index in a memory domain.
- **`docs/storage`.**

## Related

[312](312-the-minds-memory.md), [312-STO-0035](../issues-done/312-STO-0035-a-text-index-derived-from-a-journal.done), [312-STO-0038](../issues-done/312-STO-0038-assertions-about-objects.done), [310](310-the-index-on-the-medium.md), [310-STO-0048](310-STO-0048-the-longest-free-runs-kept.md), [900-NRL-0017](900-NRL-0017-native-runtime-and-nnd-integration.md).
