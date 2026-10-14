# 550-DRV-0029 — `bcm_wifi` receives OFDM from weak access points, and tells the station so

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md), through [550-DRV-0006](550-DRV-0006-broadcom-wifi.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.3

## Problem

On the MacBook Pro, `bcm_wifi` receives CCK (802.11b) frames well. OFDM frames (802.11g) come through only from strong senders: access points at −43 dBm answered our 6 Mbit/s probe requests, but their OFDM acknowledgements were not received ([550-DRV-0028](550-DRV-0028-bcm4331-transmit-power.md), runs of 2026-10-14).

Two things follow:
- **The radio says 2.4 GHz without OFDM.** `bcm_wifi` reports `bands` = 1, so the station `wlan` keeps to 802.11b rates. With `idl/wlanhw.wit` 1.1, `BAND_2GHZ_OFDM` (4) in `info.bands` tells it otherwise.
- **The test asked for blocks the network.** With `data/wifitest.txt`, `bcm_wifi` serves the station only after its whole test, so the station and the stack wait for it (requests-NET.md, "The stack waits for the station…").

## Findings so far

The 550-DRV-0028 runs, 2026-10-14:
- **Stage 3 matches b43.** It matches b43's HT-PHY init register for register, and every table value for value. b43's HT path itself sets no AGC and runs no receive calibration.
- **Not the cause:** which core transmits, CDD, how the gain is split between the radio and the baseband, and PHY table 2's gains.
- **The cause is core 2.** With core 2's analog front end switched off, access points at −43 and −44 dBm acknowledged every 6 Mbit/s request, nearly at the first try. Switching off core 0 or core 1 did not help.
- **Core 1 receives about 30 dB weaker than cores 0 and 2.**
- **Core 2's front end is now off by default** in the radio, on `claude/ASR-DRV` and `fast-test` until its gate.

## Plan

1. **Confirm on the MacBook Pro:** OFDM at 6, 12, 24 and 54 Mbit/s with core 2 off, against every network heard. This is the run on `fast-test` 5b0eb1e or later.
2. **Report OFDM to the station:** set `BAND_2GHZ_OFDM` in `info.bands` once `idl/wlanhw.wit` 1.1 is on `main`, and serve 1.1.
3. **Look for why core 2 spoils OFDM,** and whether a receive calibration or its gain setting lets it take part again. Also look at core 1's weak signal.
4. **Serve the station while the test runs, or answer it at once,** so the test no longer holds up the network.

## Acceptance criteria

- **OFDM from weak senders.** On the MacBook Pro, OFDM acknowledgements and frames from access points at −45 dBm or weaker are received in most tries, at 6 to 54 Mbit/s.
- **802.11g for the station.** `bcm_wifi` reports `BAND_2GHZ_OFDM`, and `wlan` joins a WPA2 network as an 802.11g station.

Evidence for this machine only (MC-12.1).

## Progress

- **2026-10-14: plan item 4, the station answered during the test** (`bcm_wifi/src/serve.rs`, on `claude/ASR-DRV` and `fast-test` until its gate). With `data/wifitest.txt` and a station to serve, `bcm_wifi` answers the station's requests while the test's stages run:
  - `info` is answered as it is (the SPROM's MAC address, 2.4 GHz, channels 1–13, no channel set);
  - `set-channel`, `set-filter` and `send` answer `busy`;
  - `receive` answers `empty`, and `wait` answers none.

  So `wlan` starts and keeps its loop, and `netstack` no longer waits for the test's end. After the test the radio is served as before.
- **2026-10-14, the eighth run, `fast-test` f1f30f6 (plan item 1): OFDM is received with core 2 off.** Directed requests, acknowledged of 3, with core 2's front end off:

  | Network (channel, signal) | 1M | 6M | 12M | 24M | 54M | 6M, core 0 alone | 6M, all cores on |
  |---|---|---|---|---|---|---|---|
  | `Tenda_E25140` (3, −38) | 3 | 3 | 3 | 3 | 3 | 3 | 1 |
  | `David_2.4G` (1, −43) | 3 | 3 | 3 | 3 | 0 | 3 | 1 |
  | `un2` (1, −44) | 3 | 3 | 2 | 3 | 3 | 3 | 0 |
  | a network at 4A:5D:F8:22:99:39 (6, −44) | 3 | 3 | 3 | 3 | 2 | 3 | 0 |
  | `Xiaomi_7C34` (6, −45) | 3 | 2 | 3 | 3 | 2 | 2 | 0 |
  | `TP-LINK_POCKET` (6, −38) | 0 | 0 | 0 | 0 | 0 | 0 | 0 |

  - **Every OFDM acknowledgement counted reached the host as a good OFDM frame**, at about −40 to −49 dBm on core 0 and 30 dB less on core 1.
  - **Core 0 alone does as well as cores 0 and 1.**
  - **With every core on, OFDM fails again.**
  - **`TP-LINK_POCKET` answers nothing on channel 6,** as in every run there.
- **Plan item 2 waits for `idl/wlanhw.wit` 1.1 on `main`** (`BAND_2GHZ_OFDM` is in `libmind/src/network.rs` on `fast-test` only). The criterion's first half is met at −45 dBm in most tries, up to 24 Mbit/s and mostly at 54.
- **The same boot: `wlan` got the radio's `info` 2 s after it started** (`[WLAN] RADIO B8:F6:B1:1B:0F:07, 13 CHANNELS, 2.4 GHZ 802.11B`), during the test, and `bcm_wifi` served it after the test. No scan was asked of the station afterwards: no program holds its configuration client yet (the tools track's part, `SLOT_WIFI`).

## Related

[550-DRV-0028](550-DRV-0028-bcm4331-transmit-power.md), [550-DRV-0027](550-DRV-0027-bcm4331-frame-service.md), requests-NET.md (the station's rates; the stack's wait), `bcm_wifi/`.
