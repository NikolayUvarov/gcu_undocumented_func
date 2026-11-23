# NET ready: 801-NET-0095 (a radio that answers `busy` loses no frame and moves no rate)

**Ready:** `claude/801-NET-0095-a-busy-radio-holds-the-frame` fe5591a1ce

It is built on 801-NET-0094's branch: both change `wlan/src/main.rs`.

**Advances:** 801-NET-0095, DRV's 1440Z step 1. The Ryzen run on a busy channel is left.

**Tested:**
- `wlan` builds for x86 and aarch64.
- `dot11`'s tests pass, 24 with a new case.
- `wlansim` gets a test-only feature `busy`, built by `scripts/x86_fixtures.sh`.
- The `net` suite's new `busy_radio_check`, in QEMU x86 on 4 CPUs, locally:
  - before the change: the join failed (`NO ANSWER MIND-TEST IN 60 S`);
  - with it: the station joined, leased and answered every ping.
- The whole `net` suite ran earlier on the tree with 0094, but not again with 0095.
