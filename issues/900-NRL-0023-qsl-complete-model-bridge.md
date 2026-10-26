# 900-NRL-0023: Executable QSL overlays bound to a complete supported model

**Type:** implementation, `NRL` · **Owner:** Codex (`codex/NRL-lab`) · **Priority:** P1 · **Status:** planned (2026-10-26) · **Blocked by:** — · **Roadmap:** G; 900 T04 · **Constitution:** MC-5.2, MC-11.1, MC-11.11, MC-12.1, MC-12.2, MC-12.3, MC-12.4

## Problem

The maintainer requested the five autonomous extensions after the first T11 preparation and host T12/T13 results. Existing host evidence does not establish this extension or native execution.

## Plan

Bind QSL layer replacements to an exact supported decoder base and explicit mapping, preserving external provenance and all untouched weights/tokenizer/state. Export and reload a self-contained executable model. Refuse unsupported base architectures.

## Acceptance criteria

- [ ] Verified source/base/layer geometry bindings; original QSL writer fixture; full logits agree with independent reference; standalone package works after removal of base and overlay inputs; corruption and wrong-base refusals.
- [ ] Document the supported profile and limitations, run affected checks on the track branch, preserve reports, and submit an updated landing request.

## Related

- [900](900-neural-representation-lab.md), T04.
- [NRL](../nrl/README.md). Native T11 remains a separate APP dependency.
