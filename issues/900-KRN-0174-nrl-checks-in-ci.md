# 900-KRN-0174 — NRL's host checks and portable builds in CI and in the gate

**Type:** CI, scripts · **Owner:** `KRN` · **Priority:** P2 · **Status:** open (landed in round 39; CI's run left) · **Blocked by:** — · **Main task:** [900](900-neural-representation-lab.md) (NRL's) · **Constitution:** MC-12.1, MC-12.2

**Asked by:** the neural representation lab (`NRL`) in `issues/requests-KRN.md`, 2026-10-25, with updates of 2026-10-26: "add its explicit checks to the appropriate CI/host-test entry points when the NRL branch is integrated". NRL changes no script of the kernel track's itself.

## Problem

- **`nrl/` is a Cargo workspace of its own,** with:
  - host tests;
  - portable libraries (`nrl-repr`, `nrl-runtime`) for both bare-metal targets;
  - a host runner whose pipelines replay committed fixtures.
- **None of the existing entry points discovers it.**
  - `scripts/host_tests.sh` runs a fixed list.
  - CI's jobs and `scripts/ci_local.sh`'s groups name what they run.
- **Until now the integrator ran `bash nrl/tools/check.sh` by hand** on each candidate that carried NRL's commits (landing round 37: 95 tests, both targets, the pipelines; under a minute with the crates cached).

## Plan and what was done

- **`scripts/ci_local.sh`:** a host group `NRL checks` running `bash nrl/tools/check.sh`.
  - `scripts/land.sh gate` runs it as every other group, on the host part's lane: no preset names it.
- **`.github/workflows/ci.yml`:** a job `nrl` that installs the pinned toolchain and runs the same script.
- **The request** leaves `issues/requests-KRN.md`.
- **What this is not** (as NRL says):
  - a cross-built library is not a native OS test;
  - the `nnd` smoke test inside MIND Core stays APP's integration request.

**Tested:** on the round 37 candidate (4eec3404) and on this branch, `bash nrl/tools/check.sh` passes locally: 95 tests, both bare-metal targets, the pipelines.

## Acceptance criteria

- `scripts/ci_local.sh --list` names `NRL checks`, and the group passes locally.
- CI's `nrl` job passes on the branch's push.
- The next gate runs it.

## Progress (2026-10-28)

Landed in round 39 (`main` 8721ef86, the gate on its code 381d1e85): the gate ran the group "NRL checks" on un-rizen and it passed (145 s). Left: CI's `nrl` job passing on a push; GitHub's run of the candidate (run 1633) was still queued when it landed, as on the rounds before.

## Related

[900](900-neural-representation-lab.md), `nrl/tools/check.sh`, `scripts/ci_local.sh`, `.github/workflows/ci.yml`.
