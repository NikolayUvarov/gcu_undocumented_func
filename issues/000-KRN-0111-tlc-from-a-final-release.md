# 000-KRN-0111 — The models' TLC from a final release: v1.8.0's `tla2tools.jar` is rebuilt in place

**Type:** script, third-party tool · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress (made on the branch; the gate left) · **Blocked by:** — · **Main task:** — · **Constitution:** MC-12.1, MC-12.9

## Problem

The gate of b8a7b3d on un-rizen-service, 2026-10-14: `models (TLC)` failed with `tla2tools.jar: unexpected SHA-256`.

- **The script fetched a different file.**
  - `scripts/model_check.sh` (issue 167) fetches `tla2tools.jar` from TLA+'s `v1.8.0` release and checks the SHA-256 it was written with: `7beec0f0…`, 4,501,960 bytes (2026-10-07).
  - That release is a rolling pre-release, and its asset is rebuilt in place. On 2026-10-14 the same URL gives 4,502,081 bytes with `0f56d6a8…`.
- **Who passes and who fails.** A machine that fetched the jar before keeps the old file in `~/.cache/mind-tla` and passes, as i9 and this session's container do. A machine fetching it now fails, as un-rizen-service did.
- **The check did its job.** The hash refused a file nobody had reviewed. What was wrong is the choice of a release whose asset changes.

## Plan

1. **Fetch the final release `v1.7.4`.** TLC 2.19 of 2024-08-08, 2,274,532 bytes, SHA-256 `936a2620…e88`, kept as `tla2tools-1.7.4.jar`.
2. **Drop `-noGenerateSpecTE`.** TLC 2.19 does not know the option, and the work runs in a scratch directory anyway.
3. **THIRD_PARTY.md** names 1.7.4.

## Acceptance criteria

- [x] **The four checks give the same verdicts with 1.7.4.** Run in this session's container, 2026-10-14, in 5 minutes:
  - `CapRevokeMove`: no error, 6,263,047 distinct states;
  - `RevokeFlush` for x86 and for aarch64: no error;
  - the model before 167: `OneWriter` violated, as expected.
- [ ] `models (TLC)` passes in the gate on a machine without the jar in its cache.

## Related

167 (the models), [THIRD_PARTY.md](../THIRD_PARTY.md), `docs/assurance/`.
