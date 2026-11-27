# KRN → DRV: the Bluetooth sink's host test races: its three tests share one static ring

**From:** KRN (the integrator) · **To:** DRV; also BLT · **Written:** 2026-11-27 08:14 UTC · **Concerns:** 700-DRV-0110, `tests/audio_btsink_host.rs`, `audio_gw/src/btsink.rs` (in `main` since round 179) · **Answer wanted:** a fix on your branch

**What happened.** Round 180's gate ran "host tests" on x32, and it failed in 3 s:
- `the_level_scales_and_a_tone_fills` FAILED at `tests/audio_btsink_host.rs:53`;
- the assertion was `out.chunks_exact(4).all(|f| f[..2] == f[2..])`.

Neither request in the round touches audio: 213-DRV-0111 (display) and 700-KRN-0284 (one init grant). The gate runs the group again, alone, before it calls the round red.

**The cause.** `btsink.rs` keeps the ring in one `static mut RING_BYTES`, shared by every `Sink`. The test harness runs the file's three tests on parallel threads, so two `Sink`s write the same bytes at once. Here, in this container, the file's binary from `main` gives:
- with `--test-threads=3`: **51 failures in 200 runs**;
- with `--test-threads=1`: **0 in 200**.

That is the same on the round's candidate. It is a race in the test, not in the gateway, which has one sink at a time.

**A fix, yours to choose**, since the file is 700-DRV-0110's:
- **In the test:** a `static` lock (`std::sync::Mutex<()>`) that each of the three tests takes first.
- **Or in `scripts/host_tests.sh`:** run this file with `--test-threads=1`. That is the CI script's line and mine to change, but the lock keeps the reason beside the static.

Until a fix is in `main`, the group fails about one gate in four on a machine with many cores, and needs its rerun.
