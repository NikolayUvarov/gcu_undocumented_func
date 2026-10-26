# 900-NRL-0006 — Cubic Hermite gated blocks

**Type:** research and implementation, `NRL` · **Owner:** Codex session (`codex/NRL-lab`) · **Priority:** P1 · **Status:** in progress · **Blocked by:** — (earlier tasks are on the NRL branch) · **Roadmap:** G; issue 900 T03 · **Constitution:** MC-5.1, MC-8.5, MC-11.8, MC-12.1, MC-12.2, MC-12.3, MC-12.4

## Problem

The cubic reference representation uses Hermite segments with finite-difference slopes and linear tails. It is not a B-spline, and linear interpolation alone does not cover it.

## Plan

1. Add allocation-free Hermite sampling and analytic input/coefficient derivatives, retaining the reference's endpoint and tail conventions.
2. Extend the gated block, training and standalone artifact path with an explicit format transition.
3. Generate external cubic block and standalone spline fixtures from the unchanged reference.
4. Exercise all parameters, two-knot grids, exact knots, both tails, continuity and refusals.

## Acceptance criteria

- [ ] Forward, input/coefficient and full block gradients agree with external PyTorch data.
- [ ] Independent finite differences and explicit knot/tail/continuity checks pass.
- [ ] Cubic controls retain the same source-channel mapping; SGD and exact save/load work.
- [ ] The format transition is documented; no earlier in-project compatibility reader is added.
- [ ] Host checks and both portable release builds pass on the NRL branch; no B-spline or real-model benefit is claimed.

## Related

- [900](900-neural-representation-lab.md), T03.
- [0005](../issues-done/900-NRL-0005-pruning-and-shared-channel-selection.done).
- [NRL](../nrl/README.md).
