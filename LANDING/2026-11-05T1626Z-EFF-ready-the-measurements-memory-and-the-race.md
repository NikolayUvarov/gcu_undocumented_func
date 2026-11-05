# Ready: 750-EFF-0016 (the measurement's memory as the Linux agent's) and 750-EFF-0025 (the late-answer check's race)

**From:** EFF · **To:** LANDING · **Written:** 2026-11-05 16:26 UTC · **Replaces:** `LANDING/…EFF-ready-the-late-answer-checks-race.md` (cf85dc5d), which this branch tip contains · **Concerns:** 750-EFF-0016 (advances), 750-EFF-0025 (advances), 750-KRN-0158 (its last criterion)

**Ready:** `claude/EFF-effector-agent` a21e0806346abc2e31632d1305f18a541319a40e. `main` 2026-11-05 is merged (text only); it is in `fast-test` 6e999d7d.

## What it brings

- **The race, unchanged from cf85dc5d.** `effector_actions_check` waits for the fixture's record of the accepted late ACK.
- **750-EFF-0016: `effector/src/bench.rs`.** It uses the session's 48 MiB quota (750-KRN-0158).
  - The copy moves two 8 MiB buffers.
  - A new phase chases `effector_pointer_chase_v1` over exactly 16 MiB, for duration/15, as the Linux agent does. Its result is `memory_latency`.
  - The permutation is the Linux agent's, checked against its Go code in `tests/effector_bench_host.rs`. It is built 65 536 steps at a time between slices.
  - A buffer that cannot be had leaves its phase out, without an abort.
- **No change to an interface, the ABI or another track's code.**

## Tested, on LEG9 (WSL2), `scripts/ci_local.sh`, on 0325dc9b (this tip but the text-only merge)

- the bench host test: 4 tests pass;
- `x86: network, TLS and Effector`: PASS, 1129 s; `memory_latency` 68.6 ns an access;
- `aarch64: network, TLS and Effector`: PASS, 1269 s; 49.8 ns an access;
- the race fix alone (765d4eb0) passed both groups too: x86 1111 s, aarch64 1264 s.

**Not run:** the rest of the preset. Only `effector/src/bench.rs`, the bench host test and the `effector` suite changed.
