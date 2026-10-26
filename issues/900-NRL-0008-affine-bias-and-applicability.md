# 900-NRL-0008 — Affine biases and explicit transform applicability

**Type:** research and implementation, `NRL` · **Owner:** Codex session (`codex/NRL-lab`) · **Priority:** P1 · **Status:** in progress · **Blocked by:** — (0001 through 0007 are on this branch) · **Roadmap:** G; issue 900 T01/T03 · **Constitution:** MC-5.1, MC-8.5, MC-11.8, MC-12.1, MC-12.2, MC-12.3, MC-12.4

## Problem

The five numerical paths currently support only bias-free SiLU gated blocks. General import must not discard biases, reinterpret another activation or silently select a different device. Their exact applicability and affine extension are needed before T04.

## Plan

1. Support optional independent gate/up/down output biases in compact and factored projections, including accumulated parameter gradients and SGD.
2. Preserve biases during channel packing, shared spline initialization and SVD conversion; explicitly version affine channel scoring.
3. Describe the supported block semantics, source activation, dtype/layout, state and parameter-sharing rules in the artifact; refuse unsupported requests and devices.
4. Replace the previous artifact schema without compatibility readers. Keep original reference fixtures and add a clearly marked independent PyTorch affine extension.
5. Check every bias gradient numerically, mixed/absent biases, conversion preservation, malformed inputs, round trips and both portable builds.

## Acceptance criteria

- [ ] All five affine block forward/backward/SGD cases agree with external PyTorch tensors, and every trainable bias has an independent numerical gradient check.
- [ ] Pruning and spline controls retain identical biases/indices; SVD adds each bias after its L*R product and full-rank conversion preserves outputs.
- [ ] Shapes, finite values, accumulation, optional/mixed biases and unchanged-source-on-error are exercised.
- [ ] A versioned descriptor and documented capability matrix reject unsupported semantics, activation, dtype/layout, state, parameter sharing and device without fallback.
- [ ] The new schema replaces earlier versions with explicit refusals; exact round trips and bounded host checks pass.
- [ ] All touched tests and both portable release rlibs pass on the NRL branch, with dated evidence and no native/full-model claim.

## Related

- [900](900-neural-representation-lab.md), T01 and T03.
- [0007](../issues-done/900-NRL-0007-factorized-svd-blocks.done).
- [NRL](../nrl/README.md).
