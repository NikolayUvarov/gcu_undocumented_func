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
- **2026-10-14: plan item 2, `bands` = 5** (`BAND_2GHZ | BAND_2GHZ_OFDM`, `idl/wlanhw.wit` 1.1's bit 2, whose wire is 1.0's), in both `info` answers. The answer during the test counts too, because `wlan` asks once, when it starts. This answers the network track's request, now taken from `requests-DRV.md`. Since `main` 3be9671 brought 1.1, it is written with `mind::network`'s constants.

- **2026-10-21: CoolNet2's OFDM is not received at all,** with core 2's front end off ([550-DRV-0036](550-DRV-0036-the-radios-traffic-in-the-log.md)).
  - Over seven periods of 10 s on channel 8, the radio received 210 to 428 good CCK frames each period, no good OFDM frame, and 2 to 8 OFDM frames with a bad PLCP header.
  - This network's signal was not measured in that run.
  - Until OFDM comes through, the station is offered 802.11b only ([550-DRV-0037](550-DRV-0037-802-11b-until-ofdm-is-received.md)). So this task's criterion "802.11g for the station" is met only with `data/wifi-ofdm.txt`.

- **2026-10-21: what the next run with `data/wifi-ofdm.txt` will tell.** Two things differ from the runs of 2026-10-14, where OFDM came through:
  - **the channel:** CoolNet2 is on channel 8, and those runs probed networks on channels 1, 3 and 6 only. Channel 8's values in the radio's and the PHY's tables follow the same steps as the channels around it, so a slip in one entry is not likely, though not ruled out;
  - **the frames:** those runs counted acknowledgements, 14 bytes; the access point's data to the station is longer. A receiver that keeps the start of a frame and loses its later symbols decodes the one and not the other.

  Until now a frame with a bad FCS never reached `bcm_wifi` outside the test, so "no good OFDM" could not be told apart from "no OFDM heard". Now, with `data/wifi-ofdm.txt` (`bcm_wifi/src/serve.rs` `ofdm_lines`, `radio.rs` `Counts`):
  - frames with a bad FCS or PLCP are kept and counted, not queued for the station;
  - every period that logs traffic adds two lines: OFDM frames good and with a bad FCS by rate (6 to 54 Mbit/s) and by length (up to 64 bytes, 65 to 256, longer), and the mean power by core of each, with how many were clipped;
  - **`cores-off=<0 to 7>` in the file** switches off the front ends of the cores in that mask (core 0 is bit 0) in place of core 2's alone: `cores-off=6` leaves core 0 alone, `cores-off=0` every core on. The boot logs the cores it switched off.

  What each outcome would mean:
  - **long frames with a bad FCS, short ones good:** the frame's start is received and its later symbols are not. Next: the receive gain held through a frame, IQ balance, frequency tracking;
  - **almost nothing at OFDM, good or bad, at a power that should be received:** the receiver does not detect OFDM on channel 8. Next: `--wifitest`'s probes there, and channel 8's tables against b43's source;
  - **good frames with another core set:** the cores, as on 2026-10-14.

## Related

[550-DRV-0028](550-DRV-0028-bcm4331-transmit-power.md), [550-DRV-0027](550-DRV-0027-bcm4331-frame-service.md), requests-NET.md (the station's rates; the stack's wait), `bcm_wifi/`.
