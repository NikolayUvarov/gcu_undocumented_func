# 550-DRV-0027 — `bcm_wifi` as a service: raw 802.11 frames for the station, through an IDL interface agreed with NET

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 · **Status:** open · **Blocked by:** the interface's agreement with `NET` ([requests-NET.md](requests-NET.md), "The raw-frame interface…"); [550-DRV-0026](../issues-done/550-DRV-0026-bcm4331-transmits.done) for transmitting · **Main task:** [550](550-network-on-real-hardware.md), through [550-DRV-0006](550-DRV-0006-broadcom-wifi.md) · **Constitution:** MC-2.3, MC-6.1, MC-12.4

## Problem

`bcm_wifi` runs its stages once, on request, and resets the core. The 802.11 station that `NET` is asked for (`wlan`, [requests-NET.md](requests-NET.md)) needs a driver that keeps the radio running and passes frames both ways, as step 4 of [550-DRV-0006](550-DRV-0006-broadcom-wifi.md) says.

## Plan

- **An interface,** proposed to `NET` as `idl/wlanhw.wit` 1.0 (the name and the methods are `NET`'s to agree):
  - `info`: the MAC address, the bands and channels the hardware supports;
  - `set-channel`: a 2.4 GHz channel at 20 MHz;
  - `set-filter`: whether beacons of every network pass (for a scan), the BSSID to follow, the station's own address;
  - `send`: one 802.11 frame (without FCS) at a given rate, and its transmit status;
  - `receive`: the next frames, each with its signal (dBm), channel and rate, from a shared buffer;
  - `set-key` and `clear-key`: CCMP keys for the chip's own encryption, if the station uses it (later; software CCMP needs none).
- **The driver as a service:** the radio brought up at boot (the stages' sequences, no longer on request), the receive ring kept running, the interrupt used once its routing to PCIe is settled (a configuration write `bcm_wifi` cannot make, a request to `KRN` then), and the service endpoint for `wlan` only (MC-2.3).
- **`init`:** the endpoint lent to `wlan` alone, a request to `KRN` when the station exists.

## Acceptance criteria

On the MacBook Pro, `wlan` (or, until it exists, a test client) sets a channel, receives the beacons around and sends a probe request through the interface; the frames and their signals match stage 4's log. QEMU has no such chip: the interface's decoders are fuzzed on the host like every other (500-ASR-0001).

## Progress

- **2026-10-12: `NET`'s interface is `idl/wlanhw.wit` 1.0** (550-NET-0012, still on `NET`'s branch). Its station `wlan` (550-NET-0018) calls `info`, `set-channel`, `set-filter`, `send` and `receive`, and builds every frame itself, its sequence number and duration included. So the driver sends a frame as it is given: the microcode's hardware sequence number is not used.
- **The radio's calls in `bcm_wifi/src/radio.rs`**, the server's half without the dispatch:
  - the rings of stages 4 and 5 kept running;
  - received frames filtered as the interface says, each queued with its 8-byte header (signal, channel, rate);
  - any frame sent at a CCK or OFDM rate, with the acknowledgement and the try count from its transmit status;
  - the channel set, and the filter set, with the own address and the BSSID in the hardware's address match.
- **Stage 4 is now a test of those calls**, run on request (`data/wifitest.txt`):
  - a scan as the station sees it;
  - broadcast probe requests at 1 and 6 Mbit/s;
  - a probe request to each of the three strongest networks, which must acknowledge it, at gain entries 80 and 40 (for [550-DRV-0028](550-DRV-0028-bcm4331-transmit-power.md)).
- **What remains:**
  - the dispatch of `wlanhw` requests on the service endpoint, once `idl/wlanhw.wit` and its generated code are on `main`;
  - the radio brought up at boot without the test's waits;
  - from `KRN`, an endpoint for `bcm_wifi` and its loan to `wlan`.

## Related

[550-DRV-0006](550-DRV-0006-broadcom-wifi.md), [550-DRV-0026](../issues-done/550-DRV-0026-bcm4331-transmits.done), [requests-NET.md](requests-NET.md) (the station), [550-APP-0033](550-APP-0033-wifi-setup-program.md) (`wifi`), `bcm_wifi/`.
