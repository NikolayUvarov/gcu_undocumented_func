# 900-NRL-0005 — Pruning and shared channel selection

**Type:** research and implementation, `NRL` · **Owner:** Codex session (`codex/NRL-lab`) · **Priority:** P1 · **Status:** in progress · **Blocked by:** — (builds on 900-NRL-0001 through 0004 on the same branch) · **Roadmap:** G; issue 900 T03 · **Constitution:** MC-5.1, MC-8.5, MC-11.8, MC-12.1, MC-12.2, MC-12.3, MC-12.4

## Problem

The first block profile has no pruned control. Comparing a narrow spline block to a wide dense block without using the same retained channels cannot isolate the activation representation's effect. Landing of the earlier slice is not a development dependency: the maintainer explicitly authorized continued work on this branch on 2026-10-26.

## Plan

1. Specify deterministic weight-only and activation-calibrated channel ranking, explicit calibration bounds and stable tie handling.
2. Add a no_std scoring/selection path and a host transform shared by pruned and spline candidates; preserve projection orientation and reject unsupported source semantics.
3. Version the block artifact when adding retained-channel provenance, replacing the previous reader as the project's pre-release rule requires.
4. Compare selected indices and packed block numerics with externally generated reference fixtures, and independently test gradients, full-width identity and refusals.

## Acceptance criteria

- [ ] Both ranking modes, stable ties and retained-index ordering are documented and tested; f64 scoring is distinguished from the reference's forced f32 scoring.
- [ ] The pruned SiLU block and spline controls retain exactly the same source channels and serialize their mapping.
- [ ] Forward, backward and one training step match stored external tensors; numerical gradients and exact artifact round trips pass.
- [ ] Unsupported shapes, selections, nonfinite/overflowing scores, incompatible versions and bounds fail explicitly.
- [ ] Host tests and both portable builds pass on the NRL branch; reports state limits and no model-level benefit is claimed.

## Related

- [900](900-neural-representation-lab.md), T03.
- [NRL](../nrl/README.md).
- [0004](../issues-done/900-NRL-0004-first-linear-spline-block.done).
