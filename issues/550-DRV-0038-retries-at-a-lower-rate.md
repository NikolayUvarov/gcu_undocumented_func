# 550-DRV-0038 — `bcm_wifi` sends a frame's retries at the rate one step lower

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 · **Status:** in progress · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md), through [550-DRV-0006](550-DRV-0006-broadcom-wifi.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.3

## Problem

**Sending is where frames are lost now.** On 2026-10-21 the MacBook Pro was offered 802.11b only ([550-DRV-0037](550-DRV-0037-802-11b-until-ofdm-is-received.md)), and receiving stopped losing frames. Over six periods of 10 s ([550-DRV-0036](550-DRV-0036-the-radios-traffic-in-the-log.md)):
- **at 11 Mbit/s:** 14 data frames sent, 7 acknowledged, the other 7 lost after their 4 tries;
- **at 5.5 Mbit/s:** 15 sent, 14 acknowledged.

**Why the station keeps losing them.** `wlan` steps down after two frames lost in a row and up again after ten acknowledged, so it keeps returning to 11 Mbit/s and its losses.

**Why the retries do not help.** The transmit header names a fallback rate for a frame's retries: its PHY control word at 0x0C, its encoding at 0x14 and its PLCP header at 0x36 (b43's `phy_ctl1_fb`, `extra_ft`, `plcp_fb`). `bcm_wifi` gave it the frame's own rate, so every try went at the rate that failed.

## Plan

- **The fallback rate is one step below,** as b43 picks it (`b43_calc_fallback_rate`): 11 → 5.5 → 2 → 1 Mbit/s, and 54 → 48 → 36 → 24 → 18 → 12 → 9 → 6 → 5.5.
- **Which tries go at it** is the microcode's choice: the header gives it the fallback. The run measures what it does.

## Acceptance criteria

- [ ] **On the MacBook Pro with CoolNet2, 802.11b,** 550-DRV-0036's lines show it:
  - data frames at 11 Mbit/s lost far less often than one in two;
  - `ping -c 20` to the gateway answered at least 19 times.
- [ ] **Nothing else goes wrong:** the join, DHCP and frames at 1 Mbit/s are as before.

Evidence for this machine and network only (MC-12.1). That the microcode uses the fallback for the retries is b43's documentation of the header, not measured here yet (MC-12.3).

## Progress

- **The baseline,** before this change (`fast-test` ffd5dcc, 802.11b): `ping -c 20` to the gateway answered 15 of 20.
- **2026-10-21, written** (`bcm_wifi/src/radio.rs`, `fallback`). It builds. The Mac's run is left.

## Related

[550-DRV-0036](550-DRV-0036-the-radios-traffic-in-the-log.md), [550-DRV-0037](550-DRV-0037-802-11b-until-ofdm-is-received.md), [550-DRV-0028](550-DRV-0028-bcm4331-transmit-power.md).
