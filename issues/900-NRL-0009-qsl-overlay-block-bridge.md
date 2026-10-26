# 900-NRL-0009 — QSL overlay block bridge and provenance-preserving block sets

**Type:** research and implementation, `NRL` · **Owner:** Codex session (`codex/NRL-lab`) · **Priority:** P1 · **Status:** in progress · **Blocked by:** — (0001 through 0008 are on this branch) · **Roadmap:** G; issue 900 T01/T04 (first import slice) · **Constitution:** MC-5.1, MC-8.5, MC-11.2, MC-11.8, MC-12.1, MC-12.2, MC-12.3, MC-12.4

## Problem

There is no Rust bridge from the unchanged QSL overlay format. Import must verify tensor bytes and shapes, preserve the original record/licence, distinguish an unverified base identity, and never invent source-channel indices that QSL did not save. A set of standalone numerical blocks is not a runnable standalone language model.

## Plan

1. Specify a bounded F32 safetensors reader, strict QSL overlay validation and a versioned generic block-set package with source hashes and explicit scope.
2. Import all five representations to f64, exactly widening the stored F32 values, with no Python at run time and no implicit file paths, downloads or model execution.
3. Extend the block mapping contract to represent unrecorded source channels explicitly, replacing the previous NRL schema without compatibility readers.
4. Save/load self-contained block sets with hashes, original source manifest, explicit unverified base identity and required licence information.
5. Generate a synthetic external QSL overlay, compare imported outputs, exercise round trips, malformed tensors/manifests/offsets/hashes/paths and resource bounds.

## Acceptance criteria

- [ ] All five representations import and match the saved reference parameters' f64 execution; missing retained indices remain explicitly unknown.
- [ ] The input checksum, declared byte count, QSL artifact ID, tensor shapes/names/dtype/offsets and total limits are checked; malformed/duplicate JSON and unused tensors are refused.
- [ ] Saved block-set packages round-trip exactly in numerical behaviour and verify all referenced files; no existing output is overwritten.
- [ ] The source manifest/hash, patch hash, declared base identity, numeric widening policy and caller-provided licence are retained; no base/model authenticity is claimed.
- [ ] The format/version/limit contracts and CLI are documented, with passing host checks and portable rlibs on the NRL branch.
- [ ] T04 remains open for whole-model graphs, QSL standalone models and overlay/standalone model export; this issue closes only the declared block bridge.

## Related

- [900](900-neural-representation-lab.md), T01 and T04.
- [0008](../issues-done/900-NRL-0008-affine-bias-and-applicability.done).
- [NRL](../nrl/README.md).
