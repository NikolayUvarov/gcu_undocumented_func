# 900-NRL-0018 — Measure and remove repeated decoder FFN preparation

**Type:** research and implementation, `NRL` · **Owner:** Codex session (`codex/NRL-lab`) · **Priority:** P1 · **Status:** in progress · **Blocked by:** — for host measurements; native/target numbers remain T11-dependent · **Roadmap:** G; 900 T12 · **Constitution:** MC-5.1, MC-8.5, MC-12.1, MC-12.2, MC-12.3, MC-12.9

## Problem

The checked decoder reconstructs/validates each immutable FFN view and allocates its scratch for each token. T12 must measure that cost and the complete model before calling reuse an optimization. Native results cannot be inferred from host timings.

## Plan

Keep the original per-token checked path as an explicit reference. Add immutable borrowed FFN views/scratch prepared once per document, preserving numerical operations, weight checks and public step rollback. Freeze workloads/repeats/quality conditions before measurement; compare local FFN and full-model paths in one executable with alternating order, actual work and raw timings. Verify all split logits/metrics and every compressed representation. Preserve negative/noisy outcomes; do not infer a gain from fewer allocations.

## Acceptance criteria

- [ ] A measured FFN component identifies repeated preparation cost; model/import validation and numerical checks are retained.
- [ ] Reference/prepared paths preserve full logits/traces/task metrics, model IDs, all five representations, bounds and transactional step behaviour.
- [ ] Paired same-process full-model measurements include preparation, warmups, repetitions, raw distributions, environment and exact work; quality and nonoverlap timing criteria are frozen in advance.
- [ ] Default behaviour changes only with accepted measured evidence; an opt-in/negative result remains explicit otherwise. No native/device/memory gain is inferred.
- [ ] All affected host tests, four portable builds and CLI profile cycles pass; source/evidence and repetition commands are saved.

## Related

- [900](900-neural-representation-lab.md), T12; [T08](../issues-done/900-NRL-0014-evaluate-benchmark-and-compare.done); [T11](900-NRL-0017-native-runtime-and-nnd-integration.md).
