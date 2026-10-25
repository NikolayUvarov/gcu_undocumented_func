# 900-NRL-0002 — Specify the first numerical contracts and artifact format

**Type:** research and implementation, `NRL` · **Owner:** Codex session (`codex/NRL-lab`) · **Priority:** P1 · **Status:** open · **Blocked by:** 900-NRL-0001 · **Roadmap:** G; issue 900 T01 · **Constitution:** MC-5.1, MC-5.2, MC-8.5, MC-11.2, MC-11.8, MC-12.1, MC-12.2, MC-12.3

## Problem

Issue [900](900-neural-representation-lab.md) needs a reproducible, narrowly scoped first Rust numerical path. This task carries T01 of the starting plan accepted by the maintainer on 2026-10-25.

## Plan

1. Specify a bias-free gated block with explicit input/channel/output shapes, row-major f64 tensors and numerical tolerances.
2. Specify uniform per-channel linear interpolation, fixed grids, linear tails and the derivative convention at knots.
3. Separate cubic Hermite (900 T03) from the B-spline variant requested by 262; retain both as distinct work.
4. Specify bounded data-only block artifacts, error outcomes, the host/portable boundary and APP integration.

## Acceptance criteria

- [ ] The forward and backward semantics, layouts, limits and errors can be implemented independently.
- [ ] The artifact has an explicit schema version and refuses unknown versions and fields.
- [ ] No Qwen constants, OS authority or unsupported native integration are implicit in shared contracts.

## Related

- [900](900-neural-representation-lab.md).
- [262](262-model-runtime-on-the-machines-full-hardware.md), APP's model service.
- [NRL](../docs/nrl/README.md).
