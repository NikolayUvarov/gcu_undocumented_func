# 213-DRV-0052 — RTL8852BE coexistence once the station has joined: Wi-Fi and Bluetooth share the time

**Type:** driver · **Owner:** `DRV` (with `BLT` for `bt_hci`'s scans) · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md), [550](550-network-on-real-hardware.md) · **Roadmap:** tracks H and D · **Constitution:** MC-6.1, MC-12.1, MC-12.3

## Problem

On the Ryzen (2026-11-08, [213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md), build 8e3913fd) the station joined CoolNet2 and got an address over the RTL8852BE. **Six minutes later it lost the network:**
- `bt_hci`'s round ran a 10-second inquiry (one every 320 s);
- the station heard nothing from the access point for 10 s (`SILENCE_MS`) and began to scan;
- its next frame found the BB fully in reset (`SYS_FUNC_EN` `0xDC`) and all host DMA stopped (`DMA_STOP` `0x000BFF00`), with no firmware recovery counted. The data mode stopped (`Tx(Io(Timeout))`).

**The cause.** `rtw89_wifi`'s coexistence keeps the `NoLink` policy and antenna after association, as at startup. It has only `ResetDefaults`, `NoLink` and `Rfk` (`btc_codec.rs`). Under `NoLink` the firmware gives the shared RF to Bluetooth whenever Bluetooth asks, a whole inquiry long.

**What Linux does.** After association, Linux's rtw89 tells the coexistence of the station:
- `rtw89_btc_ntfy_role_info` with `BTC_ROLE_MSTS_STA_CONN_END`;
- the role and link H2Cs.

The coexistence then picks a policy for a connected station (`_action_wl_2g_sta` and the TDMA and slot tables of `coex.c`). Wi-Fi and Bluetooth then share the time.

## Plan

1. **A check of the cause first:** a `bt_hci` with one round a day, on the Ryzen. Wi-Fi should then hold well past six minutes. This is a trial build, not committed.
2. **The role notice after the join:** from the data mode, once `wlan` reports the network joined (or on the first data frame acknowledged): the role, its link and channel, with Linux's H2C layouts from the pinned sources.
3. **A policy for a station connected on 2.4 GHz:**
   - **The idle case first:** Bluetooth idle or only scanning. Linux's TDMA and slots for it, from `coex.c`, the tables recorded with their sources.
   - **The antenna:** for a connected station, `Antenna` gains `Connected`.
4. **Leaving:** back to `NoLink` when the station leaves or the data mode stops.
5. **Until 2–4 exist:** `bt_hci`'s inquiry rounds pause while a Wi-Fi network is joined on a combined chip (`BLT`). This is a stopgap, logged as such.

## Acceptance criteria

- **The Ryzen with `bt_hci` running its rounds:** joined to CoolNet2 for an hour across at least ten inquiries, with no loss of the network and ping answered.
- **Host tests:** the role notice's encoding and the policy's TDMA and slots against Linux's values; the transitions from `NoLink` to connected and back.

## Related

[213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md), [hwdocs/rtl8852be-btc-runtime.md](../hwdocs/rtl8852be-btc-runtime.md), [700](700-bluetooth.md).
