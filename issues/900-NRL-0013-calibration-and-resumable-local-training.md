# 900-NRL-0013 — Split-aware language calibration and resumable local training

**Type:** research and implementation, `NRL` · **Owner:** Codex session (`codex/NRL-lab`) · **Priority:** P1 · **Status:** in progress · **Blocked by:** — (T06 is complete on this branch) · **Roadmap:** G; 900 T07 · **Constitution:** MC-5.1, MC-8.5, MC-9.1, MC-11.2, MC-11.3, MC-11.8, MC-12.1, MC-12.2, MC-12.3, MC-12.4

## Problem

T07 needs language activations collected only after explicit document/group splitting, transformation initialization independent of test data, local replacement training and exact checkpoint continuation. T05 replay alone is not optimizer resume.

## Plan

1. Define a bounded language corpus with sources, stable document/group IDs, predeclared splits and explicit byte-BPE/context policy. Reject cross-split duplicate documents/token sequences and groups before activations.
2. Collect bounded original-teacher FFN input/target pairs from train/validation only, retaining actual sampled documents/tokens, model/data/layer identity and row caps.
3. Initialize pruned, linear/cubic spline and SVD replacements from shared training-only channel scores/grids. Preserve the explicit pre-SVD channel map separately; keep existing artifact/SVD limits.
4. Train all supported replacement parameters with full-batch SGD and validation-only checkpoint selection. Save initial/current/best parameters, configuration/data/order, completed step/history, stateless optimizer and explicit no-RNG state for exact continuation.
5. Provide independent CLI calibration/start/resume/export entries and tests for split integrity, source/geometry/checkpoint corruption, exact resume, validation-only selection and integration with the real decoder.

## Acceptance criteria

- [ ] Corpus/group/token split checks run before activation collection; test activations never initialize or train/select a replacement.
- [ ] Calibrations bind exact teacher/data/layer/input/target/sample identities, actual row counts and declared sampling/limits.
- [ ] All four compressed controls initialize in Rust from training inputs, retain provenance and respect previous mathematical/format bounds.
- [ ] Local SGD covers every trainable projection/factor/coefficient/bias; validation alone selects the earliest best checkpoint and reports MSE/NMSE/absolute errors and grid coverage.
- [ ] Serialized optimizer/configuration/data-order/step and current/best state resume identically to uninterrupted training on the stated host; no Python is needed.
- [ ] New outputs never overwrite; invalid versions/hash/shape/source/bounds/checkpoints are refused. The selected artifact loads in the trained decoder.
- [ ] Full host checks and portable rlibs pass with saved real-model calibration/training/checkpoint evidence; T08–T10 continue next.

## Related

- [900](900-neural-representation-lab.md), T07 and sections 8, 13–15.
- [0012](../issues-done/900-NRL-0012-trained-decoder-and-tokenizer.done).
