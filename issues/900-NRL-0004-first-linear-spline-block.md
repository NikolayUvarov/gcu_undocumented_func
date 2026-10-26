# 900-NRL-0004 — Validate dense and linear-spline gated blocks against the reference

**Type:** research and implementation, `NRL` · **Owner:** Codex session (`codex/NRL-lab`) · **Priority:** P1 · **Status:** open · **Blocked by:** 900-NRL-0001, 0002, 0003 · **Roadmap:** G; issue 900 T03 (first slice); minimal T04 round trip · **Constitution:** MC-5.1, MC-5.2, MC-8.5, MC-11.2, MC-11.8, MC-12.1, MC-12.2, MC-12.3

## Problem

Issue [900](900-neural-representation-lab.md) needs a reproducible, narrowly scoped first Rust numerical path. This task carries T03 (first slice); minimal T04 round trip of the starting plan accepted by the maintainer on 2026-10-25.

## Plan

1. Implement dense SiLU and linear-spline bias-free gated blocks with explicit buffers and analytic gradients.
2. Compare forward, input and parameter gradients with reference tensors, and with independent numerical differences away from kinks.
3. Exercise knots, tails, invalid shapes/grids/sizes and finite-value handling.
4. Demonstrate one local training step and a bounded save/load round trip with a machine-readable report.

## Acceptance criteria

- [ ] Reference forward and backward agree within the documented f64 tolerances.
- [ ] Independent gradient checks, knot/tail cases and malformed-input checks pass.
- [ ] A saved block reloads with the same forward result and refuses invalid versions, data and overwrites.
- [ ] The report states actual work and remaining T03/T04 work; it claims neither useful compression nor native execution.

## Related

- [900](900-neural-representation-lab.md).
- [262](262-model-runtime-on-the-machines-full-hardware.md), APP's model service.
- [NRL](../docs/nrl/README.md).
