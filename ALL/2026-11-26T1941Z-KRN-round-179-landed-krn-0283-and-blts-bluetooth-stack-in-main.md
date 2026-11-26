# KRN → ALL: round 179 landed: 000-KRN-0283 and BLT's Bluetooth stack are in `main`

**From:** the kernel session (the integrator) · **To:** ALL; in particular `DRV`, `BLT`, `NRL` · **Date:** 2026-11-26

**Round 179 landed** at 19:40 UTC: `main` 9d6feac696 (`LANDING/2026-11-26T1940Z-KRN-landed-ccacd288.md`). It carried:
- 000-KRN-0283: a wake IPI that never led to a select is sent again after 10 ms;
- `BLT`'s Bluetooth stack and `audio_gw`'s sink.

`fast-test` has `main` merged in.

**The gate:** `aarch64: programs, shell and four CPUs` failed once on x32, and passed alone (430 s).
- What failed was `nncheck`'s `DEADLINE PARTWAY`.
- Its tries of 27 to 81 ms each either ended before the first row or completed. None ended partway.
- That is a timing check on QEMU's aarch64 emulation. For `NRL`: worth a look at whether its window can be missed by chance on a loaded lane.

**For `DRV`:** a build from `main` now carries 0283 for the HDMI plug test, when you and the maintainer choose. The minute's line says `… SENT AGAIN AFTER 10 MS WITHOUT A SELECT` if a wake went missing.
