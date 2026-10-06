# 400 — Marain M0–M2 on a host bench (track E, first step)

**Type:** main task · **Owner:** `MRN` track (open) · **Priority:** P3 · **Status:** open · **Blocked by:** — · **Roadmap:** track E "M0–M2: now (host only)" · **Constitution:** RFC 001 Marain v0.4 §15 (M0–M2), MC-11.1, MC-11.2, MC-11.7

## Problem

RFC 001 Marain v0.4 plans the language in stages. M0–M2 (specification, front end with code identities, reference evaluator) run on a host bench and need nothing from the kernel. They are the part of track E that can start now, in parallel with every other track.

## Plan

The tasks below are planned; the `MRN` track numbers them itself.

- `400-MRN-0001` — M0. A specification of the core: grammar, type, effect and ownership rules, errors, handlers and local IPC. It lives in `marain/spec/`, in English, with contradictory and negative examples, and with a comparison against the option without a new language (RFC §15 M0).
- `400-MRN-0002` — M1, parser and resolution: a host crate `marain/` with its own Cargo.lock, outside the image.
- `400-MRN-0003` — M1, type and effect checker, a canonical codec and code identities (CodeID). Test vectors: whitespace and local renaming keep the ID; a changed literal, effect or dependency changes it; a round trip keeps the definitions.
- `400-MRN-0004` — M2, a reference evaluator: affine resources and single-shot handlers. The full example of RFC §12 runs. A double resume and a use after MOVE are refused. Rights never come from the manifest.
- Changes to the RFC itself go to the maintainer as a new revision in both languages (AGENTS.md section 1).

## Acceptance criteria

- `marain/` builds and its tests pass on the host.
- A CI job, or a `scripts/ci_local.sh` group, runs them.
- RFC §15's M0–M2 acceptance conditions are each tied to a test or a document.
- Nothing here claims a guarantee of MIND Core (RFC §15: host simulation does not attest them).

## Related

`constitution/EN/RFC_001_Marain_v0.4.md` (and RU); ROADMAP track E.
