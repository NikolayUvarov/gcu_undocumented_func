# 900-NRL-0015 — Manual mixed plans and bounded measured search

**Type:** implementation, `NRL` · **Owner:** Codex session (`codex/NRL-lab`) · **Priority:** P1 · **Status:** in progress · **Blocked by:** — (T07/T08 complete on branch) · **Roadmap:** G; 900 T09 · **Constitution:** MC-5.1, MC-8.5, MC-9.1, MC-12.1, MC-12.2, MC-12.3, MC-12.9

## Problem

Independent local losses do not establish a useful combination of replacements. Manual mixed plans and bounded search must evaluate the complete student and retain negative results/dense fallback.

## Plan

Implement versioned bounded study/plan inputs, reuse train-only source calibration and local checkpoints, compose replacements simultaneously, measure each full candidate on validation, record a Pareto set and a conservative quality/time/package selection. Test is evaluated only after the selected plan is fixed. Save original inputs, candidates, selection rationale and standalone output through independent manual/search tools.

## Acceptance criteria

- [ ] A manual plan combines multiple representations/layers with explicit exact teacher/corpus/source identities and unchanged external model interfaces.
- [ ] Bounded search uses all six pre-frozen candidates and dense baseline; limits/duplicates/incompatible inputs fail clearly and candidate numerical failures are recorded.
- [ ] Full-model validation quality and actual compatible timing/package measurements determine selection; Pareto records and explicit rejection/dense fallback are retained.
- [ ] Test never constructs/fits/selects candidates; selected test evaluation occurs after selection and is recorded separately.
- [ ] CLI/manual/search packages are independently reusable with new paths and complete provenance; numerical/bounds/composition/selection checks, all host suites and portable builds pass.

## Related

- [900](900-neural-representation-lab.md), T09; [T07](../issues-done/900-NRL-0013-calibration-and-resumable-local-training.done), [T08](../issues-done/900-NRL-0014-evaluate-benchmark-and-compare.done).
