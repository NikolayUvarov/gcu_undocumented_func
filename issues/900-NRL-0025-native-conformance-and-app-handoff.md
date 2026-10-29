# 900-NRL-0025: Pinned native conformance package and nnd design handoff

**Type:** implementation, `NRL` · **Owner:** Codex (`codex/NRL-lab`) · **Priority:** P1 · **Status:** in progress (2026-10-29) · **Blocked by:** — for the host fixture/handoff; native execution remains 900-NRL-0017 · **Roadmap:** G; 900 T11 · **Constitution:** MC-5.1, MC-5.2, MC-11.1, MC-11.11, MC-12.1, MC-12.2, MC-12.3, MC-12.4

## Problem

APP answered the native request in 262-APP-0112 and docs/nn/runtime.md 0.2. It needs a pinned MIT standalone model, independently computed reference outputs and an explicit tolerance. Its text/f64 decisions need NRL confirmation and precise serialization and runtime-accounting boundaries. Native nnd, parser workers and boot grants are not implemented yet.

## Plan

1. Export an unchanged supported complete feed-forward model from the already landed MIT fixtures, all five gated paths, shared objects and independent PyTorch reference. Include verified model/file identities and little-endian f64 client/reference payloads.
2. Freeze the existing host writer encoding as a named export profile with golden f64 edge cases, while preserving the more general host model loader. Pin dtype/serializer/format versions; do not promise native readers exist.
3. Exercise work preflight/cumulative charging, partial cancellation/deadline/clock failure, finite-input numerical poisoning and independent new sessions. Keep output/workspace cleanup semantics explicit and provide native-harness guidance without claiming native evidence.
4. Verify offline regeneration/checking, integrity/refusal cases and the complete NRL profile. Send APP a concrete handoff, exact model ID, source/fixture commit, tolerance and design corrections through internal-dev-messages; notify KRN of the boot prerequisite scope.

## Acceptance criteria

- [ ] A deterministic self-contained MIT test model, SHA-bound payloads, independent reference outputs and tolerance can be regenerated and checked offline.
- [ ] Writer encoding vectors cover float signs, integer-valued f64, subnormals, exponent formatting and finite extremes; unsupported/noncanonical encodings are explicitly scoped without weakening model-v0.
- [ ] Host runtime scenarios establish the documented preflight, charging, cleanup and terminal-state behavior and named failures.
- [ ] All affected tests/four portable builds and CLI cycles pass, provenance/status are updated, and APP receives the pinned handoff. Native acceptance of 0017 remains open.

## Related

- [900-NRL-0017](900-NRL-0017-native-runtime-and-nnd-integration.md), [900](900-neural-representation-lab.md).
- [262-APP-0112](262-APP-0112-nnd-and-nn-wit-stage-0.md), [262-KRN-0178](262-KRN-0178-nnd-as-a-boot-service.md), [262-KRN-0179](262-KRN-0179-a-wait-on-a-shared-word.md).
- [APP design](../docs/nn/runtime.md) 0.2, [portable boundary](../nrl/specs/native-boundary-v0.md).
