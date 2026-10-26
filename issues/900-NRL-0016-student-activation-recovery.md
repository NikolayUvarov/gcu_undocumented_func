# 900-NRL-0016 — Multiple replacements and student-activation recovery

**Type:** implementation, `NRL` · **Owner:** Codex session (`codex/NRL-lab`) · **Priority:** P1 · **Status:** in progress · **Blocked by:** — (T07–T09 complete on branch) · **Roadmap:** G; 900 T10 · **Constitution:** MC-5.1, MC-8.5, MC-9.1, MC-12.1, MC-12.2, MC-12.3, MC-12.9

## Problem

Two replacements change the inputs seen by downstream blocks. Fitting only on the original teacher trajectory does not test adaptation to actual student activations or accumulated model-level error.

## Plan

Use the pre-frozen two-layer start plan, three rounds and learning rate. Collect train-only normalized FFN inputs from a frozen current student each round; the frozen original teacher FFN evaluated on those same inputs defines targets. Update all replacements simultaneously with local analytic SGD. Compare a matched fixed-original-input control. Select each arm's earliest minimum full-model validation NLL, then evaluate test separately. Save actual pairs, deterministic run record, whole-model metrics/timing and standalone models; verify/replay the record independently. This is local recovery with model-level selection, not end-to-end decoder backpropagation.

## Acceptance criteria

- [ ] At least two replacements have explicit source/student/corpus/target identities, bounded train-only actual student pairs and a frozen teacher; downstream student inputs differ as expected.
- [ ] Targets equal original teacher FFNs applied to student inputs; simultaneous round updates and a matched original-input control are checked independently of original-teacher calibration.
- [ ] All rounds including zero report full-model validation quality and accumulated logit error; validation alone selects the earliest best round; test follows selection.
- [ ] Saved payloads/pairs/models preserve provenance, ordering, budgets, exact repeatability on this profile and refusal of corrupt/incompatible inputs; independent CLI recovery/check/replay operations work.
- [ ] Actual same-backend dense/initial/recovered/control benchmarks and held-out results are saved honestly, including unchanged/dense fallback when quality/time budgets fail.
- [ ] Full host suites, both portable rlibs and complete CLI cycles pass; branch documentation/status and landing handoff cover T06–T10.

## Related

- [900](900-neural-representation-lab.md), T10; [T07](../issues-done/900-NRL-0013-calibration-and-resumable-local-training.done), [T08](../issues-done/900-NRL-0014-evaluate-benchmark-and-compare.done), [T09](../issues-done/900-NRL-0015-mixed-plans-and-bounded-search.done).
