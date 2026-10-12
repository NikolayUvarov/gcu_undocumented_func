# 550-DRV-0025 — `bcm_wifi`, stage 4: frames received by DMA, the networks around listed from their beacons

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 · **Status:** in progress (written from a fact sheet; waits for a run on the MacBook Pro after stage 3's) · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md), through [550-DRV-0006](550-DRV-0006-broadcom-wifi.md) · **Constitution:** MC-1.5, MC-6.1, MC-12.1, MC-12.3

## Problem

Stage 3 ([550-DRV-0024](550-DRV-0024-bcm4331-phy-and-radio.md)) sets the radio and the HT-PHY up for a 2.4 GHz channel, with the microcode running. Nothing received reaches the host yet: the 802.11 core writes frames into a receive ring in memory with its DMA engine and tells the host by an interrupt. `init` now gives `bcm_wifi` a 512 KiB DMA region below 4 GiB and the card's MSI (550-KRN-0091).

## Plan

`bcm_wifi`, on request and step by step as the earlier stages, after stage 3:

- **The receive ring.** One 64-bit descriptor ring and its buffers in the DMA region, the core's receive engine pointed at it, the MAC's receive path to it; no transmit engine set up.
- **The interrupt.** The core's interrupt mask set for received frames only, the MSI waited for, with a poll of the interrupt reason as a fallback.
- **The frames.** Each one's receive header (length, channel, the per-core signal) and its 802.11 header read; beacons and probe responses parsed for the network's name (SSID), its address (BSSID), channel, signal and security (RSN or WPA information present or not).
- **A scan.** Channels 1 to 13 visited for about 200 ms each with the MAC enabled and beacon promiscuity on; the networks heard logged once each.
- **Then stopped:** the receive engine disabled and its ring released before the core is reset.

The register facts (the DMA engine's registers and descriptor format, the receive header) come from a fact sheet written from Linux's `b43`, under 550-DRV-0006's rule. Nothing is transmitted: no probe request is sent, so only networks that send beacons are seen.

## Acceptance criteria

On the MacBook Pro, one boot on request logs the networks around it, each with its SSID, BSSID, channel and signal, as the maintainer can compare with another device's list. The machine runs on, and the DMA engine stops before the core is reset. Evidence for this machine only (MC-12.1).

## Progress

**2026-10-12:** `bcm_wifi/src/stage4.rs`, run after stage 3 when stage 3 ends with the MAC suspended:
- the 64-bit engine checked in the agent's I/O status (bit 12);
- an 8 KiB-aligned ring of 128 descriptors (the BCM4331 copies bit 12 of the ring's address into its status, so it stays clear) and 128 buffers of 2390 bytes, frame offset 38, the PCIe translation 0x80000000 in the high words;
- polled every 2 ms, without the interrupt: the PCIe interrupt routing for the core (PCI configuration 0x94) is a write `bcm_wifi` cannot make, and a scan does not need it;
- the index kept one descriptor short of the engine's current one;
- the station's own address cleared from the address match while receiving, so no frame is acknowledged; no probe request sent;
- channels 1 to 13, 200 ms each, beacons and probe responses parsed for SSID, BSSID, channel, signal and RSN/WPA/WEP; then the engine disabled and waited for before the core is reset.

## Related

[550-DRV-0006](550-DRV-0006-broadcom-wifi.md), [550-DRV-0024](550-DRV-0024-bcm4331-phy-and-radio.md), 550-KRN-0091 (the DMA region and the MSI), `bcm_wifi/`; the station, keys and association later: `NET` ([requests-NET.md](requests-NET.md)), and the `wifi` program: [550-APP-0033](550-APP-0033-wifi-setup-program.md).
