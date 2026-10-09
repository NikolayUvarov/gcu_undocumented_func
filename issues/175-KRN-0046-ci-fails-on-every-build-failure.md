# 175-KRN-0046 — CI and the local gate fail on every build failure

**Type:** kernel (CI) · **Owner:** `KRN` · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [175](175-audit-2026-10-09.md) · **Constitution:** MC-12.2, MC-12.9

## Problem

Audit finding A06 ([audit](../issues-audit/2026-10-09-repository-audit.md), [assessment](../issues-audit/2026-10-09-repository-assessment.md)):
- **GitHub CI.** GitHub's `run:` steps use `bash -e`, which does not apply to the left side of `&&`. A host test that does not compile is skipped, and the step passes if its last command does.
- **The local gate.** `x86_fixtures` in `scripts/ci_local.sh` builds eight kernel variants without `|| return 1`. A failed build is masked, and stale `/tmp/mind-*-target` builds can be tested.

The gate decides what reaches `main` (AGENTS.md section 4).

## Plan

- Every compile and build in the CI host step and in `ci_local.sh` fails its step: `set -e`-safe forms, or `|| exit 1` / `|| return 1` on each.
- The kernel variants are built into fresh target directories, or checked to be newer than the sources.
- A host test makes each position fail in turn. Each run must return nonzero and prevent a PASS.

## Acceptance criteria

- A deliberately broken host test, or fixture build, fails both the GitHub step and the local gate.
- A test binary that fails still fails them, as now.

## Progress

- **From the tools branch (2026-10-09, before CI was the kernel track's).** The GitHub host step's lines end in `|| exit 1`: `runtime`, `tts_host`, every test of the loop, and `voice_host`. The hole hid `tests/rtc_host.rs`, which had not compiled since 000-APP-0012; that test is fixed too.
- **Still to do here:** the Python lines of that step, `x86_fixtures` in `ci_local.sh`, fresh fixture targets, and the test that makes each position fail.

## Related

[011](../issues-done/011-reproducible-toolchain.done), [AGENTS.md](../AGENTS.md) section 4.
