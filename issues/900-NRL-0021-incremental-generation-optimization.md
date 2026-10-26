# 900-NRL-0021: Measured prepared incremental generation

**Type:** implementation, `NRL` · **Owner:** Codex (`codex/NRL-lab`) · **Priority:** P1 · **Status:** planned (2026-10-26) · **Blocked by:** — · **Roadmap:** G; 900 T12 · **Constitution:** MC-5.2, MC-11.1, MC-11.11, MC-12.1, MC-12.2, MC-12.3, MC-12.4

## Problem

The maintainer requested the five autonomous extensions after the first T11 preparation and host T12/T13 results. Existing host evidence does not establish this extension or native execution.

## Plan

Reuse checked FFN views and scratch across prefill and greedy decoding, preserve reference execution and public transactional step semantics, and profile paired generation runs.

## Acceptance criteria

- [ ] Identical full step logits and generated token IDs, including compressed paths; unchanged failure behavior; bounded paired measurements and explicit promotion decision.
- [ ] Document the supported profile and limitations, run affected checks on the track branch, preserve reports, and submit an updated landing request.

## Related

- [900](900-neural-representation-lab.md), T12.
- [NRL](../nrl/README.md). Native T11 remains a separate APP dependency.
