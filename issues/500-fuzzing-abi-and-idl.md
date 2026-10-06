# 500 — Fuzzing the system calls and the IDL decoders (Assurance, first step)

**Type:** main task · **Owner:** `ASR` track (open) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** Assurance "fuzzing of syscalls and IDL decoders; fault injection" · **Constitution:** MC-12.2, MC-2.3, MC-3.1, MC-6.1, MC-11.3

## Problem

The kernel's system calls and the generated IDL decoders are the system's widest attack surface. The tests use chosen inputs; the TLA+ models (`docs/assurance`) cover the capability tree, not the code. Nothing feeds the system calls or the decoders random or malformed input. MC-12.2 asks for implementation analysis and testing that correspond to the invariants.

## Plan

The tasks below are planned; the `ASR` track numbers them itself.

- `500-ASR-0001` — host fuzzing of every generated decoder in `libmind/src/idl/`. A deterministic, seeded generator lives in a host test: no network or extra tools needed in CI. A decoder must never panic, read out of bounds or accept a message its schema forbids. Findings go to the owning track as requests.
- `500-ASR-0002` — a system-call fuzzer program in ring 3: random numbers, arguments, handles and pointers, seeded and logged. It runs in a QEMU suite on x86 and aarch64. The kernel must never panic or fault, and other tasks keep running (MC-6.1). The heap and frame pool must return to their baseline after the fuzzer ends.
- `500-ASR-0003` — the escrow capabilities (issue 170) in the TLA+ model `CapRevokeMove`: an escrowed privilege is never usable, and revoking it removes what was granted from it.
- `500-ASR-0004` — fault injection: drivers killed in the middle of I/O, checked against the restart contract (MC-6.3, 6.6).
- Kernel bugs found go to `KRN` as requests with the seed that reproduces them. They are not fixed in `ASR` commits.

## Acceptance criteria

- The fuzzers run in CI groups with a fixed seed and a stated number of iterations.
- `docs/assurance/README.md` says what they cover. It says that a fuzzing run is evidence of a number of executions, not a proof (MC-12.2).

## Related

`docs/assurance/`, `tests/isolation_app.rs` (the hand-made system-call checks), `scripts/mind_idl.py`; issue 167.
