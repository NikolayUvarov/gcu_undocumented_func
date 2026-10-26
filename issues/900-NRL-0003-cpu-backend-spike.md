# 900-NRL-0003 — Select and exercise a narrow portable Rust CPU backend

**Type:** research and implementation, `NRL` · **Owner:** Codex session (`codex/NRL-lab`) · **Priority:** P1 · **Status:** open · **Blocked by:** 900-NRL-0002 · **Roadmap:** G; issue 900 T02 · **Constitution:** MC-5.1, MC-5.2, MC-8.5, MC-11.2, MC-11.8, MC-12.1, MC-12.2, MC-12.3

## Problem

Issue [900](900-neural-representation-lab.md) needs a reproducible, narrowly scoped first Rust numerical path. This task carries T02 of the starting plan accepted by the maintainer on 2026-10-25.

## Plan

1. Create a small isolated workspace under nrl with portable numerical crates and host tooling.
2. Run a CPU forward and one training step, and compile the portable path for the MIND Core target.
3. Record the backend decision, exact dependencies, FFI and allocation boundaries; distinguish cross-compilation from an OS run.

## Acceptance criteria

- [ ] A reproducible host command executes a forward and one finite, measured training step.
- [ ] The portable numerical crates compile without std for x86_64-unknown-none.
- [ ] The ADR records the dependency graph and limitations without claiming a full training framework or native service.

## Related

- [900](900-neural-representation-lab.md).
- [262](262-model-runtime-on-the-machines-full-hardware.md), APP's model service.
- [NRL](../docs/nrl/README.md).
