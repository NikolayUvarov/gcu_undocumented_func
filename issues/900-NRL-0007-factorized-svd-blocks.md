# 900-NRL-0007 — Factorized SVD blocks and bounded CPU initialization

**Type:** research and implementation, `NRL` · **Owner:** Codex session (`codex/NRL-lab`) · **Priority:** P1 · **Status:** in progress · **Blocked by:** — (earlier tasks are on the NRL branch) · **Roadmap:** G; issue 900 T03 · **Constitution:** MC-5.1, MC-8.5, MC-11.8, MC-12.1, MC-12.2, MC-12.3, MC-12.4

## Problem

A fair spline experiment needs a low-rank control that actually executes its factors, with their own analytic gradients. Factor orientation, rank and reconstruction must be explicit. The reference supports bias-free gated blocks; arbitrary biased conversion is not silently accepted.

## Plan

1. Add allocation-free low-rank projections and a bias-free factored SiLU gated block, including direct factor forward/backward and reconstruction checks.
2. Implement a bounded host f64 SVD initializer for small matrices using one-sided Jacobi rotations, with convergence and numerical refusals.
3. Extend the versioned artifact/training path to factors; replace older schemas explicitly.
4. Check external reference block tensors, independent differences for every factor, rectangular/rank-deficient/zero reconstruction, low-rank residuals and malformed inputs.

## Acceptance criteria

- [ ] Direct factor forward/backward and one-step training agree with external PyTorch data, and independent gradients pass.
- [ ] Shapes, orientation, rank, scratch, nonfinite data and accumulation semantics are checked.
- [ ] Full-rank reconstruction and truncated residuals agree with an independent numerical reference on declared small-matrix cases; nonconvergence is an error.
- [ ] Factor artifacts save/load exactly, refuse inconsistent tensors and old schemas, and do not silently materialize weights for execution.
- [ ] All host tests and both portable rlibs pass on the NRL branch; initialization limits, bias refusals and remaining T03/model work are explicit.

## Related

- [900](900-neural-representation-lab.md), T03.
- [0006](../issues-done/900-NRL-0006-cubic-hermite-block.done).
- [NRL](../nrl/README.md).
