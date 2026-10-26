# 900-NRL-0020: Reusable AdamW and resumable decoder recovery

**Type:** implementation, `NRL` · **Owner:** Codex (`codex/NRL-lab`) · **Priority:** P1 · **Status:** planned (2026-10-26) · **Blocked by:** — · **Roadmap:** G; 900 T07, T10 · **Constitution:** MC-5.2, MC-11.1, MC-11.11, MC-12.1, MC-12.2, MC-12.3, MC-12.4

## Problem

The maintainer requested the five autonomous extensions after the first T11 preparation and host T12/T13 results. Existing host evidence does not establish this extension or native execution.

## Plan

Implement a bounded named-tensor AdamW optimizer with bias correction, decoupled decay and complete checkpoint state. Repeat multi-layer teacher-on-student-input recovery; retain validation selection and a matched SGD control.

## Acceptance criteria

- [ ] Independent optimizer oracle; exact interrupted/resumed continuation; nonfinite/shape/state refusals are transactional; saved recovery and held-out comparison.
- [ ] Document the supported profile and limitations, run affected checks on the track branch, preserve reports, and submit an updated landing request.

## Related

- [900](900-neural-representation-lab.md), T07, T10.
- [NRL](../nrl/README.md). Native T11 remains a separate APP dependency.
