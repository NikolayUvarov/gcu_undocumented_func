# 550-DRV-0026 — `bcm_wifi`, stage 5: frames transmitted, within the power the SPROM allows

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md), through [550-DRV-0006](550-DRV-0006-broadcom-wifi.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.3

## Problem

Stage 4 ([550-DRV-0025](../issues-done/550-DRV-0025-bcm4331-receives-frames.done)) receives by DMA and lists the networks that send beacons. Nothing is transmitted yet, so no probe request (an active scan), authentication or association can happen. Stage 3 left out the transmit power steps on purpose.

## Plan

`bcm_wifi`, on request and step by step as the earlier stages:

- **Transmit power.** The per-core 2.4 GHz maximum power and PA parameters from the SPROM (revision 9), the idle TSSI measured, and the HT-PHY's power control on, as the fact sheet describes. A frame is never sent above the SPROM's maximum.
- **The transmit ring.** One 64-bit transmit ring in the DMA region; the transmit header the microcode expects (format 598 and later) in front of each frame; a fixed basic rate (1 Mbit/s CCK or 6 Mbit/s OFDM).
- **Completion.** The transmit status the microcode reports read back for each frame.
- **A test.** An active scan: one probe request with the broadcast SSID on each of channels 1 to 11 (allowed everywhere; 12 and 13 stay receive-only until the region is known), and the probe responses that answer it.

The facts come from a fact sheet written from Linux's `b43` (transmit header, rings, SPROM layout, power control), under 550-DRV-0006's rule.

## Acceptance criteria

On the MacBook Pro, one boot on request logs each probe request's transmit status and the probe responses received for it, with the transmit power used. The machine runs on. Evidence for this machine only (MC-12.1).

## Related

[550-DRV-0006](550-DRV-0006-broadcom-wifi.md), [550-DRV-0025](../issues-done/550-DRV-0025-bcm4331-receives-frames.done), [550-DRV-0027](550-DRV-0027-bcm4331-frame-service.md), `bcm_wifi/`.
