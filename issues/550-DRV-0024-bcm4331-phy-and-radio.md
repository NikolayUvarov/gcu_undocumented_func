# 550-DRV-0024 — `bcm_wifi`, stage 3: the HT-PHY and the 2059 radio set up and a channel tuned

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md), through [550-DRV-0006](550-DRV-0006-broadcom-wifi.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.3

## Problem

Stage 2 ([550-DRV-0023](../issues-done/550-DRV-0023-bcm4331-microcode-runs.done)) loaded and ran the microcode on the MacBook Pro: revision 784.2, an HT-PHY of revision 1, a 2059 radio of revision 0. Nothing can be received until the MAC has its initial values and the PHY and the radio are set up for a channel.

## Plan

`bcm_wifi/src/main.rs`, on request (`data/wifitest.txt`) and step by step as stage 2, after the microcode answers:

- **Initial values.** `ht0initvals29.fw` and `ht0bsinitvals29.fw` from `data/firmware/b43/` (`scripts/proprietary.sh`): a list of register offsets, sizes and values, written into the core.
- **The MAC.** The address from the SPROM into the receiver's address match; the MAC left suspended, the receiver off.
- **The PHY and the radio.** The HT-PHY's and the 2059's set-up for 2.4 GHz at 20 MHz, then channel 1, 6 or 11 tuned and the radio's PLL lock read back. The register sequences and tables are taken as facts from the b43 specifications and from Linux's `b43` (`phy_ht`, `radio_2059`), under 550-DRV-0006's rule: values and sequences, no code copied or translated.
- **A measurement.** The PHY's noise estimate on the tuned channel, read and logged.
- **Then stopped.** The core reset again, as stage 2 does.

## Acceptance criteria

On the MacBook Pro, one boot on request logs the initial values written, the channel tuned with the radio's PLL locked, and a noise figure that changes when the channel changes. The machine runs on. Evidence for this machine only (MC-12.1); nothing is sent.

## Related

[550-DRV-0006](550-DRV-0006-broadcom-wifi.md), [550-DRV-0023](../issues-done/550-DRV-0023-bcm4331-microcode-runs.done), stage 4: [requests-KRN.md](requests-KRN.md) ("`bcm_wifi` gets DMA memory and its interrupt"), `bcm_wifi/`.
