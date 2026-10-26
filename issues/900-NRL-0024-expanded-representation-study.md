# 900-NRL-0024: Expanded held-out representation study

**Type:** implementation, `NRL` · **Owner:** Codex (`codex/NRL-lab`) · **Priority:** P1 · **Status:** planned (2026-10-26) · **Blocked by:** — · **Roadmap:** G; 900 T08, T09, T10 · **Constitution:** MC-5.2, MC-11.1, MC-11.11, MC-12.1, MC-12.2, MC-12.3, MC-12.4

## Problem

The maintainer requested the five autonomous extensions after the first T11 preparation and host T12/T13 results. Existing host evidence does not establish this extension or native execution.

## Plan

Freeze a larger licensed corpus and experiment policy before evaluation. Repeat representation/recovery comparisons on the real decoder, reporting actual quality, package sizes and host timings, with dense fallback.

## Acceptance criteria

- [ ] Pinned data/source/licence/hashes and split boundaries; larger than miniature fixture; reproducible bounded CLI study; independent held-out reports; no unsupported quality/compression claim.
- [ ] Document the supported profile and limitations, run affected checks on the track branch, preserve reports, and submit an updated landing request.

## Related

- [900](900-neural-representation-lab.md), T08, T09, T10.
- [NRL](../nrl/README.md). Native T11 remains a separate APP dependency.
