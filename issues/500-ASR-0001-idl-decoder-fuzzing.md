# 500-ASR-0001 — Fuzzing every generated IDL decoder on the host

**Type:** assurance (host test) · **Owner:** `ASR` · **Priority:** P2 · **Status:** in progress · **Blocked by:** the CI line, a request in [requests-KRN.md](requests-KRN.md) ("The IDL fuzzer in CI's host tests") · **Main task:** [500](500-fuzzing-abi-and-idl.md) · **Constitution:** MC-2.4, MC-2.11, MC-2.12, MC-12.2

## Problem

The generated decoders in `libmind/src/idl/` are where every service meets untrusted input from its clients. Their tests use chosen messages (`tests/idl_host.rs`, `tests/idl_test.py`). Nothing feeds them random or malformed input.

## Plan

- **A host test,** [`tests/idl_fuzz_host.rs`](../tests/idl_fuzz_host.rs). It has a deterministic, seeded generator, needs no network and no extra tools, and runs the generated code unchanged.
- **Receivers.** Every interface's `decode` gets messages shaped from its generated source and mutated from those it accepted. It must:
  - not panic;
  - release a received capability (MC-2.12);
  - refuse another major version as `Version`;
  - refuse a buffer request with a trailing or a missing byte (MC-2.4);
  - keep a decoded request unchanged when the client rewrites its buffer (MC-2.11).
- **Types.** Every generated `Wire` type decodes random and mutated bytes, and must accept only canonical encodings within its `MAX`.
- **Completeness.** The test checks against the generated sources that it lists every interface and type.
- **Docs.** [docs/assurance/README.md](../docs/assurance/README.md) says what it covers, its seed and count, and that a run is not a proof.
- **Findings** go to the owning track as requests, with the seed that reproduces them.

## Acceptance criteria

- The test runs in CI's host tests with its fixed seed and its stated count. The CI files are the kernel track's, hence the request.
- It fails on an injected defect of each kind it checks.
- docs/assurance/README.md describes it.

## Progress

**2026-10-09:**
- The test is written. It covers 24 interfaces and 82 types, with 50 000 inputs per target in about 4 s.
- No finding at the fixed seed, nor at five other seeds with 200 000 inputs per target.
- Three injected defects were caught: `bool` accepting 2; `header` keeping a refused capability; a `vfs` decoder skipping its end-of-request check.
- docs/assurance/README.md 1.1 describes it.
- What remains is the CI line.

**2026-10-10:** `tests/idl_fuzz_host.rs` and docs/assurance/README.md 1.1 are in `main` (8e181ad, after a local gate of all 31 groups). What remains is the CI line, the kernel track's.

## Related

[500](500-fuzzing-abi-and-idl.md), [351-ASR-0006](351-ASR-0006-update-threat-model.md) (fuzzing the update's parsers, next), `scripts/mind_idl.py`, `tests/idl_host.rs`.
