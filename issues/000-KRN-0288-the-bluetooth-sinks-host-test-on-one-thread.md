# 000-KRN-0288 — The Bluetooth sink's host test on one thread

**Type:** CI (`scripts/host_tests.sh`) · **Owner:** `KRN` (the kernel session) · **Priority:** P1 · **Status:** in progress (made, for round 180's second gate) · **Blocked by:** — · **Main task:** — · **Constitution:** MC-12.2

## Problem

Round 180's gate failed "host tests" on x32, both runs (2026-11-27). The failing case was `the_level_scales_and_a_tone_fills` in `tests/audio_btsink_host.rs`, at the assertion that both channels of a tone are alike. Neither request in the round touches audio.

`audio_gw/src/btsink.rs` (700-DRV-0110, in `main` since round 179) keeps its ring in one `static mut`, as the gateway has one sink at a time. The test harness runs the file's three cases on parallel threads, so two sinks write the same bytes at once. The file's binary from `main`, run in a container:
- with `--test-threads=3`: 51 failures in 200 runs;
- with `--test-threads=1`: none in 200.

It is a race between the test's cases, not in the gateway.

## Plan (made)

`scripts/host_tests.sh` builds `audio_btsink` on a line of its own, as `OWN_LINES` allows (000-KRN-0238), and runs its cases with `--test-threads=1`. No case is skipped or changed. DRV was told (`DRV/2026-11-27T…-KRN-the-bluetooth-sinks-host-test-races-…`). A lock in the test file would let this line go; the test file is DRV's.

## Acceptance criteria

- [x] On one thread the file's binary passes 100 runs in 100, in this container.
- [ ] The gate's "host tests" group passes.

## Related

- 700-DRV-0110: the sink.
- 000-KRN-0238: the host tests' own lines.
