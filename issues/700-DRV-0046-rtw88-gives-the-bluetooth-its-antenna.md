# 700-DRV-0046 — `rtw88_wifi` lets the card's Bluetooth on air: rtw88's free run on a card whose Bluetooth has its own antenna

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 · **Status:** in progress (2026-10-31) · **Blocked by:** — · **Main task:** [700](700-bluetooth.md) · **Constitution:** MC-12.1, MC-12.3

## Problem

- **The symptom.** On the DEXP, `bt_hci` (700-BLT-0001) patched the RTL8821C and listened, but heard nothing for ten hours (2026-10-30):
  - each round, `0 LE AND 0 BR/EDR DEVICES`;
  - with 700-BLT-0002's counts, 0 events in a 10 s LE scan;
  - an inquiry that ended by itself after 7.0 s, status 00, with no result.
- **The cause.** `rtw88_wifi/src/coex.rs` set the coexistence up as Linux rtw88 does for a card without Bluetooth (`__rtw_coex_init_hw_config` with `wifi_only`, `COEX_SET_ANT_WONLY`):
  - `GNT_BT` software low, `GNT_WL` software high (LTE coexistence register 0x38, 0x7700);
  - the switch at WLG.

  So the Bluetooth was never granted the air.
- **What Linux does on this card.** It uses `wifi_only` only when the eFuse says the card has no Bluetooth (`rtw_power_on`). The DEXP's eFuse says `BT COEX true SHARED ANTENNA false`, RFE 0. For a Bluetooth on its own antenna, rtw88's runtime action is the free run (`rtw_coex_action_freerun`):
  - `COEX_SET_ANT_2G_FREERUN`: `GNT_BT` by the hardware PTA, `GNT_WL` software high, the switch at WLG_BT;
  - table 100 of `table_nsant_8821c`.

## Plan

1. **`coex::Board`** gains `bt` (the eFuse's `btcoex`); `Board::mode` gives the free run for a card with Bluetooth on its own antenna, else the Wi-Fi alone.
2. **`coex::init`** (was `wifi_only`), in the free run:
   - 0x38's grant fields `0x3300` (`GNT_BT` 0, the PTA; `GNT_WL` 3, software high) instead of `0x7700`;
   - 0xcb4[31:28] = 3 (WLG_BT; 2 on module types 2 and 4) instead of 2;
   - the PTA's tables 0x6c0 and 0x6c4 = 0xffffffff, and the break table 0x6c8 = 0xf0ffffff.
3. **A card with Bluetooth on a shared antenna keeps the Wi-Fi-only set-up.** It needs rtw88's arbitration by the Bluetooth's state (BT info from the firmware, tables and TDMA by case), which this driver does not run.
4. **The log names the mode:** `[RTW88] ANTENNA: FREE RUN, …` or `THE WI-FI'S ALONE`.

**Not done:** rtw88's free run also sets the Wi-Fi's RF parameters by the RSSI (`rtw_coex_set_rf_para`) and tells the Bluetooth's firmware the Wi-Fi's channel (`rtw_coex_update_wl_ch_info`, for its AFH). Neither is needed to hear.

## Acceptance criteria

- [ ] The x86 build and all of `scripts/host_tests.sh` pass.
- [ ] On the DEXP, with this `rtw88_wifi`:
  - the log says the free run;
  - Wi-Fi joins and works as before;
  - `bt_hci`'s rounds hear devices.

  Evidence for the DEXP only (MC-12.1).

## Progress (2026-10-31)

**On the DEXP,** release 202610301115 (ABI 25, tree `ff0da96d`), by `effector485 replace`, unverified. Both services were built from that tree with this change and 700-BLT-0002's applied, since `main` is at ABI 26.
- **The log:** `[RTW88] ANTENNA: FREE RUN, THE BLUETOOTH ON ITS OWN ANTENNA GRANTED BY THE PTA` at 43021 s.
- **The Wi-Fi joined again by itself:** `DHCP 192.168.2.141/24` 14 s after the restart. The agent reconnected to hesere.
- **Then:** every frame sent was acknowledged (`SENT 30 (30 ACKED)`, `21 (21 ACKED)` in 10 s tallies).
- **`bt_hci`'s first round after it:** `15 LE AND 1 BR/EDR DEVICES HEARD; 25 EVENTS IN THE LE SCAN, 69 IN THE INQUIRY`, LE levels −56 to −76 dBm, and `BR/EDR D8:A3:5C:9C:36:BB RSSI -72 CLASS 0C043C AUDIO`.
- **Just before it,** with the Wi-Fi-only set-up: 0 events in the LE scan, 1 in the inquiry.

A restart of the DEXP returns to the boot images until a release carries this.

## Related

[700-BLT-0001](700-BLT-0001-bt-hci-the-rtl8821c-up.md), [550-DRV-0042](../issues-done/550-DRV-0042-rtl8821ce-wifi.done) (the driver), [hwdocs/rtl8821ce.md](../hwdocs/rtl8821ce.md). Linux v6.12 rtw88 `coex.c` and `rtw8821c.c`, as `coex.rs` names them.
