# 900-NRL-0010 — Executable feed-forward graphs and complete model packages

**Type:** research and implementation, `NRL` · **Owner:** Codex session (`codex/NRL-lab`) · **Priority:** P1 · **Status:** in progress · **Blocked by:** — (0001 through 0009 are on this branch) · **Roadmap:** G; issue 900 T01/T04; prerequisite for T05 · **Constitution:** MC-5.1, MC-8.5, MC-9.1, MC-11.2, MC-11.3, MC-11.8, MC-12.1, MC-12.2, MC-12.3, MC-12.4

## Problem

A block collection does not describe a complete computation. T04 needs an explicit bounded graph, exact source identity, real overlay/standalone semantics and preserved provenance before an independent model adapter can run end to end.

## Plan

1. Add a no_std dense affine primitive with explicit output biases and analytic gradients.
2. Define a versioned narrow feed-forward DAG, model/adapter/preprocessor/task descriptor and enforced scalar f64 execution profile. Reject unsupported operators, states, shapes and invalid references.
3. Represent weight sharing explicitly by object identity, validate declared aliases and count each shared object once.
4. Save/load complete standalone models and parameter-replacement overlays bound to a verified standalone base model; export a resolved overlay to a standalone model without the base directory.
5. Preserve source licences/notices and full change history; derive transform parameters and content identity from checked bytes. Require all dependencies to be supplied explicitly and refuse unsupported chains, paths and old versions.
6. Add external full-graph oracles, end-to-end CLI checks, corruption/bounds/identity tests and both portable builds.

## Acceptance criteria

- [ ] The complete supported graph executes against an independent external oracle, including rectangular affine layers, gated blocks, activations, residual addition and explicit shared objects.
- [ ] Graph/shape/cycle/reachability, object inventory, alias, input, dtype/backend and memory bounds are checked before execution.
- [ ] Standalone packages run with no source directory; overlays contain only changed objects, require the exact verified base and reject wrong/missing bases or shape changes.
- [ ] Resolving/exporting an overlay produces a standalone package with identical outputs and preserved licences/notices/change history; no existing output is overwritten.
- [ ] Data-only formats and content hashes/IDs bind graph, parameters, numerical policy and metadata; identity is not presented as authenticity.
- [ ] Host tests, independent affine gradients, package/CLI checks and both portable release rlibs pass on the NRL branch with dated evidence.
- [ ] Remaining QSL whole-model/language/native work stays explicit; this is a bounded feed-forward profile, not a universal graph compiler.

## Related

- [900](900-neural-representation-lab.md), T01, T04 and T05.
- [0009](../issues-done/900-NRL-0009-qsl-overlay-block-bridge.done).
- [NRL](../nrl/README.md).
