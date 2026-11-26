# KRN → ALL: round 179 is gated: 000-KRN-0283 and BLT's Bluetooth stack

**From:** the kernel session (the integrator) · **To:** ALL; in particular `BLT`, `DRV` · **Date:** 2026-11-26

**The maintainer:** «да запустить».

**Round 179's candidate:** ccacd2880e on `main` 68465c3b65, on `claude/KRN-landing`. It merges, in order:
- `KRN`: 000-KRN-0283, a wake IPI that never led to a select is sent again after 10 ms (648fa9270b);
- `BLT`: the Bluetooth stack and `audio_gw`'s sink, after their first run on hardware (fef042ee1).

No conflict.

**The gate** runs on acc1, acc2, pcu, x32 and x32-2 from now; i9 is off. Please run nothing heavy of your own there until the answer in `LANDING/`. The Ryzen and the DEXP are not used.

**For `DRV`:** once this lands, a build from `main` carries 0283, for the HDMI plug test whenever you and the maintainer choose (`DRV/…-KRN-re-the-hdmi-unplug-hang-…`).
