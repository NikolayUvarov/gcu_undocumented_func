# 700-DRV-0114 — The Bluetooth sink's host test: its tests take turns with the one static ring

**Type:** test (`tests/audio_btsink_host.rs`) · **Owner:** `DRV` · **Priority:** P1 (it fails a landing gate about one time in four) · **Status:** in progress (2026-11-27: the fix and its runs are on the branch) · **Blocked by:** — · **Main task:** [700](700-bluetooth.md) · **Roadmap:** the drivers track · **Constitution:** MC-12.1, MC-12.2

## Problem

KRN, the integrator, 2026-11-27 (`DRV/2026-11-27T0814Z-KRN-the-bluetooth-sinks-host-test-races-its-tests-share-one-static-ring.md` on `internal-dev-messages`): round 180's gate failed "host tests" on x32 at `the_level_scales_and_a_tone_fills` (`tests/audio_btsink_host.rs:53`), though neither request in the round touches audio.

The cause is the test's, not the gateway's. `audio_gw/src/btsink.rs` keeps the ring in one `static mut RING_BYTES`, as the gateway holds one sink at a time. The test harness runs the file's three tests on parallel threads, and two `Sink`s then write the same bytes. On LEG9, before the fix: 37 failures in 200 runs with `--test-threads=3`.

This came with 700-DRV-0110 ([done](../issues-done/700-DRV-0110-a-bluetooth-sink-in-the-audio-gateway.done)), in `main` since round 179.

## Plan

A `static` lock in the test file that each of its three tests takes first, beside a comment that says why. The gateway's code is not changed: it has one sink, and the static is right there. `scripts/host_tests.sh` is not changed either, so the reason stays beside the static.

## Acceptance criteria

- [x] On LEG9, the file's binary with `--test-threads=3`: 0 failures in 200 runs, against 37 before.
- [x] Its three tests pass, and nothing else in them changes.
- [ ] In `main`, and a landing gate's "host tests" passes without the rerun this caused.

## Related

[700-DRV-0110](../issues-done/700-DRV-0110-a-bluetooth-sink-in-the-audio-gateway.done) (the sink), [700](700-bluetooth.md).
