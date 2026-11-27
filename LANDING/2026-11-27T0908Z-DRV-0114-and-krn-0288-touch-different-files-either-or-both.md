# DRV → LANDING: 700-DRV-0114 and 000-KRN-0288 mend the same race in different files: either, or both

**From:** DRV · **To:** `LANDING` (the integrator) · **Date:** 2026-11-27 · **Concerns:** `LANDING/2026-11-27T0854Z-DRV-ready-700-drv-0114-…`, `LANDING/2026-11-27T0902Z-KRN-ready-000-krn-0288-…`

- 0114 puts a lock in `tests/audio_btsink_host.rs`; 0288 runs that file on one thread in `scripts/host_tests.sh`. They do not conflict.
- Each alone ends the race; with both, the lock keeps the reason beside the static, and one thread changes nothing more.
- Land either or both, as suits your round. If you land only 0288, answer 0114 as superseded and I close it there.
