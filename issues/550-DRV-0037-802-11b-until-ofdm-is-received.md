# 550-DRV-0037 — `bcm_wifi` offers the station 802.11b only, until it receives OFDM

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 · **Status:** in progress · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md), through [550-DRV-0006](550-DRV-0006-broadcom-wifi.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.3

## Problem

On 2026-10-21, on the MacBook Pro joined to CoolNet2, the radio received no good OFDM frame in 70 s, against hundreds of good CCK frames ([550-DRV-0036](550-DRV-0036-the-radios-traffic-in-the-log.md)).
- **What the station offered:** 802.11g, because `bcm_wifi` reports `BAND_2GHZ_OFDM` ([550-DRV-0029](550-DRV-0029-bcm4331-receives-ofdm.md)).
- **What follows:** the access point sends to it at OFDM rates, and those frames are lost until it retries them at CCK rates.
- **What the maintainer saw:** pings answer 2 of 3, and DHCP takes 50 s.

## Plan

1. **`bcm_wifi` reports 2.4 GHz without `BAND_2GHZ_OFDM`** unless the boot volume holds `data/wifi-ofdm.txt`. `wlan` then offers and uses 802.11b's rates (550-NET-0028), and the access point keeps to CCK: 1, 2, 5.5 and 11 Mbit/s.
2. **`data/wifi-ofdm.txt` offers 802.11g again,** for 550-DRV-0029's runs. Each boot logs which it offers.
3. **When 550-DRV-0029 receives OFDM** from the maintainer's access points, 802.11g becomes the default again.

## Acceptance criteria

- [ ] **On the MacBook Pro with CoolNet2, without the file:**
  - the join and the DHCP lease within a few seconds;
  - `ping -c 20` to the gateway answered at least 19 times;
  - 550-DRV-0036's lines show the data received as CCK.
- [ ] **With the file,** the station is offered 802.11g as before.

Evidence for this machine and network only (MC-12.1). 802.11b's rates bound the throughput to well under 11 Mbit/s (MC-12.3).

## Progress

- **2026-10-21, written** (`bcm_wifi/src/serve.rs` `offer_ofdm`, `bcm_wifi/src/main.rs`). It builds.
- **2026-10-21, the MacBook Pro** (`fast-test` ffd5dcc), joined by hand to CoolNet2, then `ping -c 20`:
  - **Receiving:** 483 to 523 good CCK frames each 10 s, and no OFDM frame at all, good or bad. The access point kept to CCK.
  - **DHCP took 15 s** after the join, where it had taken 50 s.
  - **Sending is now where frames are lost.** At 11 Mbit/s 7 of 14 data frames were lost after 4 tries; at 5.5 Mbit/s 1 of 15. The next task is [550-DRV-0038](550-DRV-0038-retries-at-a-lower-rate.md).
  - **`ping -c 20` to the gateway: 15 of 20 answered,** where pings had answered about 2 of 3 with 802.11g. The criterion of 19 is not met, so it stays open for 550-DRV-0038.
- **2026-10-21, the MacBook Pro again** (`fast-test` d2daa430, with 550-DRV-0038), rejoined by `autorun.msh` at start (550-ASR-0007):
  - `[BCM] 2.4 GHZ OFFERED TO THE STATION: 802.11B ONLY, CCK`;
  - **the join and the lease:** joined 1.7 s after the script asked; DHCP's lease 0.5 s after the join;
  - **receiving:** 465 good CCK frames in the first 10 s, none bad, and no OFDM frame; 389 to 443 in the later periods;
  - **the script's three pings of the gateway** all answered, 13 to 28 ms.

  Of the first criterion, the join, the lease and the CCK lines are met on this machine and network; `ping -c 20` on this build is left.

## Related

[550-DRV-0036](550-DRV-0036-the-radios-traffic-in-the-log.md), [550-DRV-0029](550-DRV-0029-bcm4331-receives-ofdm.md), [550-NET-0028](550-NET-0028-rates-the-radio-has.md).
