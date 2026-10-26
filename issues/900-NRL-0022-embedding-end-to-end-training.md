# 900-NRL-0022: End-to-end embedding training and checkpoints

**Type:** implementation, `NRL` · **Owner:** Codex (`codex/NRL-lab`) · **Priority:** P1 · **Status:** planned (2026-10-26) · **Blocked by:** — · **Roadmap:** G; 900 T07, T13 · **Constitution:** MC-5.2, MC-11.1, MC-11.11, MC-12.1, MC-12.2, MC-12.3, MC-12.4

## Problem

The maintainer requested the five autonomous extensions after the first T11 preparation and host T12/T13 results. Existing host evidence does not establish this extension or native execution.

## Plan

Differentiate lookup, mean pooling, checked projection and L2 normalization; train a bounded retrieval objective using shared AdamW, validation selection and standalone export.

## Acceptance criteria

- [ ] Independent full gradient oracle, repeated-token accumulation, deterministic checkpoint continuation, no test-based selection, saved train/validation/test retrieval reports.
- [ ] Document the supported profile and limitations, run affected checks on the track branch, preserve reports, and submit an updated landing request.

## Related

- [900](900-neural-representation-lab.md), T07, T13.
- [NRL](../nrl/README.md). Native T11 remains a separate APP dependency.
