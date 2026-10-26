# 900-NRL-0019 — Mean-embedding adapter and retrieval task profile

**Type:** implementation, `NRL` · **Owner:** Codex session (`codex/NRL-lab`) · **Priority:** P1 · **Status:** in progress · **Blocked by:** — for host; native execution remains T11 work · **Roadmap:** G; 900 T13 · **Constitution:** MC-5.1, MC-8.1, MC-8.5, MC-9.1, MC-11.11, MC-12.1, MC-12.2, MC-12.3, MC-12.9

## Problem

A new family must supply its own inputs/state/preprocessing/metrics while reusing established representation mathematics. Language perplexity is not a retrieval metric. Existing host adapters do not implement token lookup, mean pooling and normalized embeddings.

## Plan

Add a bounded stateless mean-token-embedding family with an exact vocabulary, shared checked feed-forward projection and unit normalization. Reuse existing gated transforms and model packaging; keep lookup/pooling/normalization outside the transform inventory. Provide independent complete encoder packages and source/history/identity checks. Add fixed-index/query-group split-aware retrieval with explicit Recall@k, MRR and graded NDCG@k, compatible shared evaluation/benchmark/compare reports and independent PyTorch golden vectors/rankings. No pretrained quality, full-family training or native execution is implied by the synthetic fixture.

## Acceptance criteria

- [ ] New family has explicit bounded data-only config/vocabulary/table/projection/preprocessing/state/tasks, independent package/identity/provenance and clear unsupported/zero-vector refusals.
- [ ] Lookup/mean/projection/L2 and actual retrieval vectors/scores/rankings/metrics match an independent PyTorch reference; existing mathematical kernels are reused unchanged.
- [ ] Shared five-representation projection replacement/export works with stable external boundaries; source/notice/history survives and incompatible input semantics fail.
- [ ] Retrieval validates fixed index/query groups, split/token-bag duplication and relevance; evaluate/benchmark/compare use the same backend and declared work/measurement semantics.
- [ ] Malformed/corrupt/unknown versions/bounds, round trips, source mismatch, new-path CLI cycles and all host/portable checks pass; reproducible evidence and limitations are saved.

## Related

- [900](900-neural-representation-lab.md), first T13 extension; [T08](../issues-done/900-NRL-0014-evaluate-benchmark-and-compare.done), [T11](900-NRL-0017-native-runtime-and-nnd-integration.md), [T12](../issues-done/900-NRL-0018-prepared-decoder-experiment.done).
