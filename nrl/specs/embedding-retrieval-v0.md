# Mean-embedding family and retrieval profile v0

Serves 900 T13; MC-5.1, MC-8.1, MC-8.5, MC-9.1, MC-11.11, MC-12.1–12.3, MC-12.9. This is the first additional bounded host family, not an ONNX/Transformer embedding implementation or universal model IR.

## Family and artifacts

`nrl.embedding-definition/0` contains a versioned table/vocabulary, existing complete feed-forward projection definition, maximum token length and source declarations. `nrl.embedding/0` is a standalone package: fixed `table.json`, checked standalone `projection/`, and final `manifest.json`. Fixed paths, canonical manifest/table encodings, sizes/hashes, nested projection identity, versions, geometry and source/licence/notices are checked. Unknown formats, fields, non-regular files, corruption, unsupported external boundaries and missing inputs are refused. Hashes establish integrity/identity, not authenticity of source declarations. Nothing follows source URLs or executes model code.

`nrl.mean-embedding` version 0 is stateless. Its exact case-sensitive ASCII whitespace vocabulary has no unknown-token substitution, BOS/EOS, normalization or truncation. Each sequence has 1–128 tokens; vocabulary at most 1024, lookup width/output width at most 128 and lookup values at most 16384. Compute arithmetic mean of lookup vectors, run the checked projection, then divide by its Euclidean norm. Norms at most 1e-12 and non-finite results are refused. Empty/unknown/over-budget tokens fail explicitly. Model identity binds configuration, vocabulary/table, projection, sources and previous projection IDs. Source notices and prior projection provenance survive replacement/export.

Only existing projection objects are transform candidates. Affine/nonlinear/add/tied objects and five gated representations use the existing checked model/representation kernels without a new mathematics implementation. Manual `replace-embedding-projection` accepts only a matching standalone projection; `prune-embedding` uses the existing weight-energy pruning and overlay/standalone history. Lookup/pooling/normalization are unchanged. This adapter has no complete lookup/pooling/normalization backward/training or automatic retrieval optimizer; those capabilities are explicitly unsupported. The fixture is original synthetic data, not pretrained or representative semantic-quality evidence.

## Retrieval task

New named task profile `embedding-retrieval/0` uses the unchanged `nrl.evaluation/0` envelope. Documents form a fixed index shared across query splits; queries have independent train/validation/test groups and graded relevance 1–3. Limits are 32 documents and 32 queries, at most 64 encodings per pass. Every split is present. IDs/text/relevance are checked; groups and identical order-invariant query token bags cannot cross splits, and duplicate index token bags are refused before projection. Sharing the fixed index is explicit, not a claim that it is split training data. No fitting/selection is performed here.

Rank by descending dot product of unit vectors, then lexical document ID on equal scores. Recall@k is relevant documents in the first k divided by total relevant documents; MRR averages reciprocal rank of the first positive. DCG@k uses `(2^grade - 1)/log2(rank + 2)` for zero-based rank, normalized by descending ideal grades. The scalar shared-report metric is `ndcg-loss-at-k = 1 - clamp(mean NDCG@k, 0, 1)`, so lower remains better for the common comparison API; Recall/MRR/NDCG details are higher-is-better. k is fixed in the dataset before evaluation.

Compatibility binds exact encoder input semantics (vocabulary/table/token limit/pooling/normalization and external projection dimensions), baseline, data/split, profile, scalar f64 backend and the existing strict host/executable/measurement identities. Full quality includes encoder outputs, ranking and metrics. Measured work re-encodes every document and selected query from pretokenized IDs on every pass; pooling/projection/normalization are timed, ranking/metrics/loading/hashing are excluded. Reports declare this stateless timing scope, sequence/output counts, actual canonical package bytes and existing whole-process RSS limitations. No KV cache is imposed on this family.

## Tools and evidence

```text
import-embedding DEFINITION NEW_PACKAGE
check-embedding PACKAGE
encode-embedding PACKAGE TEXT NEW_VECTOR
prune-embedding ENCODER OBJECT WIDTH SOURCE NEW_PACKAGE
replace-embedding-projection ENCODER PROJECTION SOURCE NEW_PACKAGE
evaluate-retrieval BASELINE ENCODER DATA SPLIT NEW_REPORT
benchmark-retrieval BASELINE ENCODER DATA SPLIT MEASUREMENT NEW_REPORT
compare-reports BASELINE_REPORT CANDIDATE_REPORT NEW_COMPARISON
```

Every output path is new; interrupted directories without the final manifest are incomplete. Optional `tools/generate_embedding_fixture.py --output-dir tests/fixtures/embedding-v0 --check` regenerates original fixture/config/data and independent PyTorch mean/gated-residual/affine/L2 vectors, cosine rankings and metrics. Normal Rust use requires no Python/download. Native embedding execution, native parsing/authentication/cancellation, broader model imports and end-to-end family training remain separate work; this profile does not close T11.
