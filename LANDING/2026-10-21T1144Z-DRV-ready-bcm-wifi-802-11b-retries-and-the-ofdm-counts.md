# Ready: `bcm_wifi`'s 802.11b mode, retries one rate lower, and the OFDM counts

**From:** DRV (with ASR) · **To:** LANDING · **Written:** 2026-10-21 11:44 UTC · **Replaces:** LANDING/2026-10-21T1104Z-DRV-ready-bcm-wifi-802-11b-retries-and-the-ofdm-counts.md · **Concerns:** 550-DRV-0029, 0036, 0037, 0038; 550-ASR-0007

**Ready:** `claude/550-DRV-0036-radio-counts` f32d8bc094f4f1d6e2a90383742a7fc88e0b9138

## What it brings

- **550-DRV-0037:** `bcm_wifi` offers the station 802.11b only, unless `data/wifi-ofdm.txt` asks for 802.11g. The MacBook Pro's access point sent OFDM that never came through.
- **550-DRV-0038:** a frame's retries go at the rate one step lower, as b43 picks the fallback in the transmit header.
- **550-DRV-0029, a diagnostic:**
  - with `data/wifi-ofdm.txt`, frames with a bad FCS are kept and counted, never queued;
  - OFDM is logged by rate and by length, with the mean power by core;
  - `cores-off=<mask>` in the file chooses which receive cores are off, for a test.
- **550-ASR-0007:** `tests/hardware/wifijoin.msh` joins a network `keystore` remembers when `data/wifijoin.txt` names it alone.
- **Text:**
  - the MacBook Pro's results in 0029, 0036, 0037 and 550-ASR-0007;
  - 550-DRV-0038 closed;
  - 0037's first criterion met.

## `main` in it

98586f1. It merges cleanly with `claude/LNK-link`, whose request is posted beside this one.

## What was run

- **On the link track's WSL machine:**
  - `./02_build.sh` (x86): `bcm_wifi` builds with no warning;
  - `scripts/host_tests.sh`, the SSH tests included: passed.

  No QEMU suite runs `bcm_wifi`: QEMU has no BCM4331.
- **On the MacBook Pro** (`fast-test` d2daa430):
  - the boot ran `wifijoin.msh` as `autorun.msh` and rejoined CoolNet2 by itself;
  - DHCP, then three pings of the gateway, all answered;
  - `*** WIFI JOIN Complete. Data collected ***`;
  - 802.11b received with no loss, and 11 of 12 data frames at 11 Mbit/s were acknowledged.

- **On the MacBook Pro again** (`fast-test` d8acd167), pinging over the link with `netcheck`, one echo per check:
  - the gateway answered 19 of 20 and 39 of 40 pings, and 20 of 20 after a restart;
  - 4 of 92 data frames at 11 Mbit/s were lost, where the baseline lost 7 of 14.

  The OFDM counts are not run on the Mac yet. They need an image with `data/wifi-ofdm.txt`.
