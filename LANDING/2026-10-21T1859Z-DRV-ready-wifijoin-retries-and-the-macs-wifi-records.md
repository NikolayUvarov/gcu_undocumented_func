# Ready: `wifijoin.msh` tries the join again, its host test, and the MacBook Pro's Wi-Fi records

**From:** DRV · **To:** LANDING · **Written:** 2026-10-21 18:59 UTC · **Replaces:** LANDING/2026-10-21T1833Z-DRV-ready-the-macs-wifi-records.md · **Concerns:** 550-DRV-0029, 550-DRV-0036, 550-DRV-0037, 550-DRV-0038; 550-ASR-0007

**Ready:** `claude/550-DRV-0036-radio-counts` c2b4df1c681cc1d510d4b9810e53004cd14bddbc

## What it brings

- **550-ASR-0007:**
  - `tests/hardware/wifijoin.msh` tries the join up to five times, 3 s apart: after the first update through the link, the boot's one scan missed CoolNet2;
  - `tests/wifijoin_host.rs` runs the script in the shell's own interpreter, and is in `scripts/host_tests.sh`.

Text:
- **550-DRV-0037 closed.** On the Mac with `data/wifi-ofdm.txt`, the station joined as 802.11g and the gateway answered 39 of 40 pings.
- **550-DRV-0029's counts.** Long OFDM frames never come through: over 256 bytes 2 good of 154, up to 64 bytes 78 % good, at the same power.

## `main` in it

e4fd3b3.

## What was run

- `tests/wifijoin_host.rs`: 3 passed.
- `scripts/host_tests.sh` in full, the SSH tests included, passed on this branch before this change (2026-10-21, main e4fd3b3 merged). The change adds the one test above to its list.
