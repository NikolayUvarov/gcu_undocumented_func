# 900-NRL-0001 — Inventory and reproduce the reference, and prepare differential tensors

**Type:** research and implementation, `NRL` · **Owner:** Codex session (`codex/NRL-lab`) · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Roadmap:** G; issue 900 T00 · **Constitution:** MC-5.1, MC-5.2, MC-8.5, MC-11.2, MC-11.8, MC-12.1, MC-12.2, MC-12.3

## Problem

Issue [900](900-neural-representation-lab.md) needs a reproducible, narrowly scoped first Rust numerical path. This task carries T00 of the starting plan accepted by the maintainer on 2026-10-25.

## Plan

1. Verify every archived manifest entry and the archive SHA-256; record the licence and numerical coverage.
2. Run the unchanged reference tests in an isolated pinned CPU environment, recording actual results and skips.
3. Generate deterministic tensors, outputs and gradients for a small dense and linear-spline gated block; record source and data hashes.
4. Inventory current MIND Core APIs instead of retaining superseded limits in issue 900.

## Acceptance criteria

- [ ] The unchanged archive and internal manifest validate, and their provenance is documented.
- [ ] A repeatable command and a pinned environment reproduce a recorded test result; unavailable checks stay explicit.
- [ ] Fixtures record the reference hash, settings, inputs, outputs and gradients; a repeated generation matches.
- [ ] The current ABI, memory, worker and service boundaries are recorded with source paths.

## Related

- [900](900-neural-representation-lab.md).
- [262](262-model-runtime-on-the-machines-full-hardware.md), APP's model service.
- [NRL](../docs/nrl/README.md).
