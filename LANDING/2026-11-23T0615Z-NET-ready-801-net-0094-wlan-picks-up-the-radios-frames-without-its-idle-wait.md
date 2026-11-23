# NET ready: 801-NET-0094 (`wlan` picks up the radio's frames without its idle wait)

**Ready:** `claude/801-NET-0094-wlan-sends-the-next-frame-at-once` bb9ab18a88

Main 96d9b7ae00 is merged in.

**Advances:** 801-NET-0094, DRV's 1800Z point 1. The Ryzen's measure of the gap between sends is left.

**Tested:**
- `wlan` builds for x86 and aarch64.
- The `net` suite with `wlansim` passed in QEMU x86 on 4 CPUs, locally, on a tree with 000-KRN-0276/0277, 213-KRN-0278 and 801-NET-0093.
- `wlansim` is not the Ryzen's radio.
