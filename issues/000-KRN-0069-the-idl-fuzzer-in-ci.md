# 000-KRN-0069 — The IDL fuzzer in CI's host tests

**Type:** CI · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress (made; the gate left) · **Blocked by:** — · **Roadmap:** Assurance · **Constitution:** MC-2.4, MC-2.11, MC-2.12, MC-12.2

## Problem

The assurance track's request (`requests-KRN.md`, for [500-ASR-0001](500-ASR-0001-idl-decoder-fuzzing.md)) is about `tests/idl_fuzz_host.rs`. That test fuzzes every generated IDL decoder with a fixed seed, but no CI step ran it. The CI files are the kernel track's.

## Plan

Add `idl_fuzz` to the list of host tests in `scripts/host_tests.sh`. CI's step "Host tests" and `scripts/ci_local.sh`'s group "host tests" both run that script. A failed build or a finding then fails the step (175-KRN-0046).

## Acceptance criteria

1. CI and `ci_local.sh` build and run the test.
2. It passes in the gate: about 17 s to build and 10 s to run on the kernel session's machine.

## Related

[500-ASR-0001](500-ASR-0001-idl-decoder-fuzzing.md).
