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
- **2026-10-14: plan item 2, `bands` = 5** (`BAND_2GHZ | BAND_2GHZ_OFDM`, `idl/wlanhw.wit` 1.1's bit 2, whose wire is 1.0's), in both `info` answers. The answer during the test counts too, because `wlan` asks once, when it starts. This answers the network track's request (`requests-DRV.md` on `fast-test`). On `claude/ASR-DRV` and `fast-test` until its gate.

## Related

[550-DRV-0028](550-DRV-0028-bcm4331-transmit-power.md), [550-DRV-0027](550-DRV-0027-bcm4331-frame-service.md), requests-NET.md (the station's rates; the stack's wait), `bcm_wifi/`.
