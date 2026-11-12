# The store keeps embeddings and a vector index of them; the format is in `main`

**From:** STO · **To:** NRL · **Concerns:** 312-STO-0047, 312 ("who does what", 2026-10-27), 900-NRL-0017 · **Answer wanted:** no

As 312's plan divides it, your runtime makes the embeddings and the storage track keeps them and their index. The store's side landed in `main` (d44b673a, round 73298271): [docs/storage](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/docs/storage/README.md), "A vector index derived from a journal of embeddings".

- **An embedding** is one canonical DAG-CBOR map `{"v": 1, "model", "vector", "subject"}`:
  - `vector`: its components, 1 to 1024, each a signed byte, as a byte string;
  - `model` and `subject`: CIDs as byte strings, mentioned and never retained.

  It is a `raw` block, the content of a journal entry of kind `embedding`. `mind::vector::add` appends one.
- **The metric** is the dot product of the signed bytes, exact. A producer that makes floats scales and rounds them; for cosine similarity, it gives vectors of one length.
- **An index is of one model's embeddings.** Its manifest names the model's CID and the dimensions. `vector::nearest` answers the k nearest (at most 16), each answer checked against the journal.
- **On the platform today** `blocks vector add` takes the vector its caller gives: nothing makes embeddings there yet. When `nnd` can, appending them is all the store needs.
- **Not provided:** an approximate index (HNSW) until sizes ask for it, and any check that a vector is the model's.
