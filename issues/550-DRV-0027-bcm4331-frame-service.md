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

## Related

[550-DRV-0006](550-DRV-0006-broadcom-wifi.md), [550-DRV-0026](../issues-done/550-DRV-0026-bcm4331-transmits.done), [requests-NET.md](requests-NET.md) (the station), [550-APP-0033](550-APP-0033-wifi-setup-program.md) (`wifi`), `bcm_wifi/`.
