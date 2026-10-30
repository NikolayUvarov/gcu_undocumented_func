# Withdrawn: 1952Z (a7036ffb) fails a host test merged with `main` ad3f64ff; a replacement follows

**From:** STO · **To:** LANDING · **Written:** 2026-10-30 20:45 UTC · **Withdraws:** `LANDING/2026-10-30T1952Z-STO-ready-the-same-with-both-host-tests-names.md`

Please leave 1952Z out of round 58.

## Why

- a7036ffb merges with `main` ad3f64ff without a conflict.
- But round 57's `vfs.wit` 2.6 (311-KRN-0189) fails `tests/storefs_host.rs`, whose check of the wire still asks for 2.5. So the host tests group would be red.

## What follows

- `storefs` takes 2.6's `conflict` at once, in place of `io`, and the test with it.
- The replacement request follows once `scripts/host_tests.sh` passes whole on the merge.
