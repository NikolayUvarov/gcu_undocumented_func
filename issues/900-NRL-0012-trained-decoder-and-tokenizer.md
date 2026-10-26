# 900-NRL-0012 — A real trained small decoder, tokenizer and dense language baseline

**Type:** research and implementation, `NRL` · **Owner:** Codex session (`codex/NRL-lab`) · **Priority:** P1 · **Status:** in progress · **Blocked by:** — (prior work is on this branch) · **Roadmap:** G; 900 T06 and remaining T04 language identity/import · **Constitution:** MC-5.1, MC-8.5, MC-9.1, MC-11.2, MC-11.3, MC-11.8, MC-12.1, MC-12.2, MC-12.3, MC-12.4

## Problem

T06 requires an actual trained language model, not a random tiny graph. Qwen is a proposed example; a smaller real decoder makes complete offline differential/state/tokenizer and later calibration/search runs reproducible in this host profile.

## Plan

1. Pin the MIT-declared Karpathy TinyStories-260K model/custom tokenizer and primary reference revisions/hashes; preserve licence and source declarations. No pickle/model-repository code is executed.
2. Add an explicit bounded llama2.c-v0 float32 data reader and scalar f64 decoder: embeddings/tied head, RMSNorm, GQA, stored RoPE tables, causal attention, residuals and dense SiLU gated FFNs. Existing block/graph formats and limits are unchanged.
3. Add the explicit byte-BPE tokenizer profile, source-bound KV state, context/trace budgets, data-only packages and small-layer replacement hooks for T07.
4. Compare actual trained weights against independent PyTorch f64 and original-policy f32 calculations; compare full prefixes/cached decoding, tokenization and generation; reject malformed files, wrong states and budgets.
5. Save a dense language baseline and repeat commands, distinguishing numerical parity from language quality, compression, other families and native execution.

## Acceptance criteria

- [ ] Trained weights, tokenizer, licence/source and configuration identities are pinned and checked offline; no unsafe model code executes.
- [ ] Full decoder logits on actual weights match an independent oracle under the stated numerical policy; original float32 policy comparisons are separate.
- [ ] Prefill, sequential cache, uncached prefixes, state/model binding, context and token/trace limits are checked.
- [ ] Token IDs/decoding match independent primary-reference goldens for declared byte-BPE semantics; unsupported/corrupt inputs fail clearly.
- [ ] Package/replacement boundaries, source hashes, tied-weight counts, nonfinite/truncated/trailing inputs and no-overwrite rules are checked.
- [ ] Host suite and both portable rlibs pass with a dated dense baseline; T07–T10 and native limitations remain explicit.

## Related

- [900](900-neural-representation-lab.md), T06, sections 4, 8, 10, 13–15.
- [0011](../issues-done/900-NRL-0011-feed-forward-training-and-evaluation.done).
- https://huggingface.co/karpathy/tinyllamas/tree/0bd21da7698eaf29a0d7de3992de8a46ef624add/stories260K
- https://github.com/karpathy/llama2.c/tree/350e04fe35433e6d2941dce5a1f53308f87058eb
