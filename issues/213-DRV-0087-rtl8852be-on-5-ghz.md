# 213-DRV-0087 — The RTL8852BE on 5 GHz: 20 MHz channels at 802.11a's rates

**Type:** driver (`rtw89_wifi`, its tables and their generators) · **Owner:** `DRV` · **Priority:** P1 · **Status:** open (2026-11-21) · **Blocked by:** the branch of [213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md)'s change of 2026-11-21 in `main` (the base); NET's word on `wlan` · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.3

## Problem

The driver was brought up on the narrowest configuration, as [213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md)'s plan said: 2.4 GHz, 20 MHz. Every table and calibration in it is that band's.

On the Ryzen's network, the same machine and access point, 2026-11-21:

| | 5 GHz, channel 100 | 2.4 GHz, channel 8 |
|---|---|---|
| Linux | 16.4 MB/s (VHT 80 MHz) | 480 to 826 KB/s (HT 20 MHz) |
| MIND Core | not there | 252 to 353 KB/s |

The 2.4 GHz channel is crowded, and its antenna is shared with Bluetooth. The 5 GHz one is neither.

**What is there already:** `wlanhw.wit` 2.0 has the 5 GHz bit in `bands` and room for its channels, and `dot11`'s country and station code name the 5 GHz channels and their rates. Whether `wlan` scans and joins there is asked of NET.

## Scope

- **In:** the 20 MHz channels of 5 GHz the country allows, at 6 to 54 Mb/s.
- **Not in, each a task of its own later:** HT and VHT, wider channels, aggregation. So this task will not reach Linux's 16 MB/s; what it does reach is to be measured.
- **Channels with radar rules** (52 to 144; the Ryzen's network is on 100): the station receives there and sends only after it has heard the access point's beacon on that channel. No probe request before that. The driver's calibration transmits too, so it follows the same rule: a channel is calibrated when the station first sends there, as now.

## Plan

1. **Sources and tables,** from the pinned Linux sources as for 2.4 GHz, each recorded in `THIRD_PARTY.md` and made by a generator in `scripts/hardware/` that reproduces it:
   - the baseband's and the radio's channel setting for 5 GHz;
   - the receive gain tables and the board's gain offsets for its sub-bands;
   - the transmit power by rate and the limits by regulatory domain;
   - TSSI, IQK and DPK for 5 GHz.
2. **The channel policy and the country catalogue:** the 5 GHz channels with their limits, listen-only where the rules say so; `info`'s `bands` and `supported`.
3. **The band switch:** what Linux does when the band changes, the coexistence's side included (on 5 GHz the station does not share Bluetooth's path).
4. **A reference from Linux on channel 100,** as was made for channel 8 on 2026-11-21: the registers of the MAC, the baseband, the radio and the calibration with a link up, compared with what this driver leaves.
5. **On the Ryzen, a step at a time:**
   - a receive-only listen on channel 100 hears the access point's beacons;
   - the calibration there completes;
   - the station joins the 5 GHz network;
   - 6 MB from the LAN, measured as on 2026-11-21 (the sender's `TCP_INFO`), beside the 2.4 GHz figure of the same hour.
6. **2.4 GHz stays as it is:** its host cases pass unchanged, and the Ryzen still joins on channel 8.

## Acceptance criteria

- [ ] Host cases cover the 5 GHz channel setting, the policy and its limits, and that nothing is sent on a radar channel before a beacon is heard there. The generators reproduce the tables.
- [ ] On the Ryzen the scan lists the network on channel 100, the station joins it, and 6 MB arrive at a recorded rate. This holds for that machine, that access point and that channel (MC-12.1).
- [ ] The Ryzen still joins on channel 8 with the same build, and its rate there is recorded.
- [ ] The issue says what was not done: other channel widths, HT and VHT, other machines.

## Related

[213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md) (the driver; its section of 2026-11-21 has the measurements and the method), [213-DRV-0052](213-DRV-0052-coexistence-after-association.md) (coexistence), [213-KRN-0122](213-KRN-0122-platform-power-policy-for-the-wlan-device.md) (the platform's power policy).
