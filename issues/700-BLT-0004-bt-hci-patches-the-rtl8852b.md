# 700-BLT-0004 — `bt_hci` patches the Ryzen's RTL8852B: its "RTBTCore" file, and fragment indices past 255

**Type:** driver (`bt_hci`) · **Owner:** `BLT` · **Priority:** P1 · **Status:** in progress (2026-11-20) · **Blocked by:** — · **Main task:** [700](700-bluetooth.md), [801](801-the-bench-stays-manageable.md) · **Constitution:** MC-6.1, MC-12.1

## Problem

The Ryzen's Bluetooth controller, the RTL8852BE's `0BDA:B85B`, ran on its ROM. `bt_hci` patched the DEXP's RTL8821C alone and said `NOT AN UNPATCHED RTL8821C: NO PATCH`.

**What the ROM did:**
- **Its scans heard nothing.** In the 2026-11-20 rounds: `0 LE AND 0 BR/EDR DEVICES HEARD; 0 EVENTS IN THE LE SCAN, 1 IN THE INQUIRY`.
- **It sent no BT_INFO** through the Wi-Fi firmware. `rtw89_wifi`'s coexistence (213-DRV-0052) never heard what the Bluetooth did, an inquiry included, nor that it ignored the WLAN.
- **Its inquiries left the Wi-Fi deaf** until `rtw89_wifi` restarted (700-BLT-0003). That is why `bt_hci` listens only when asked.

**Why the RTL8821C's loader could not take it:**
- **The file format.** The patch, linux-firmware's `rtl_bt/rtl8852bu_fw.bin`, is in another format ("RTBTCore", sections of parts by ROM version) than the RTL8821C's ("Realtech").
- **The fragment index.** Its image is 267 fragments. The index rule `bt_hci` used gave the 256th fragment 0x80, which marks the last fragment.

## Plan

1. **The format.** `rtl::parse_core` and `rtl::image_core` read the "RTBTCore" format from its layout ([hwdocs/rtl8852b-bt.md](../hwdocs/rtl8852b-bt.md)): the parts for the ROM version, in rising priority. A security section is refused, and so are two parts of one priority.
2. **The index.** Fragment indices run 0, then 1 to 0x7F over and over.
3. **The chip.** `bt_hci` knows the RTL8852B (LMP 0x8852, HCI revision 0x000B, project 20) beside the RTL8821C. Its patch is `data/firmware/rtl_bt/rtl8852bu_fw.bin`, checked by size and SHA-256, with no configuration file.
4. **The file.** [`scripts/firmware/rtl8852b_bt.py`](../scripts/firmware/rtl8852b_bt.py), run by `scripts/proprietary.sh fetch`, fetches it from the same pinned linux-firmware revision.

## Acceptance criteria

- [x] Host tests (`tests/bt_hci_host.rs`): the "RTBTCore" choice, order and refusals; the indices at fragments 0xFE, 0xFF, 381, 382 and the last; the fetched file's two parts (67,133 and 60,101 bytes).
- [x] On the Ryzen: `RTL8852B PATCHED`, a new LMP subversion, and devices heard in an LE scan and an inquiry.
- [ ] On the Ryzen: the Wi-Fi keeps its network through the patched controller's inquiries, with `rtw89_wifi`'s coexistence hearing its BT_INFO. Otherwise the listening stays off by default (700-BLT-0003), and 213-DRV-0052 takes what is left.
- [ ] The DEXP's RTL8821C is still patched (`RTL8821C PATCHED`).

## Progress (2026-11-20): on the Ryzen

This was a test build of `bt_hci` on the ABI 34 tree, put in with `replace` over the cable. The file went in with `put` into the link's `firmware/rtl_bt/`.
```
1434.895 bt_hci: [BT] PATCH DOWNLOADED IN 555 MS
1434.930 bt_hci: [BT] RTL8852B PATCHED: LMP SUBVERSION 4E04, HCI REVISION 42D3
1434.952 rtw89_wifi: [RTW89] BT INFO [27, 06, 00, 00, FF, 28, 00, 00]: INQ/PAGE 0, INQUIRY 0, PAGE 0, IGNORES WLAN 1, LE SCAN 1, REINIT 0, CONNECTED 0
1434.952 rtw89_wifi: [RTW89] BT IGNORED THE WLAN: TOLD NOT TO
1445.092 rtw89_wifi: [RTW89] BT INFO [27, 06, 04, 00, FF, 00, 00, 00]: INQ/PAGE 1, INQUIRY 0, PAGE 0, IGNORES WLAN 0, LE SCAN 0, REINIT 0, CONNECTED 0
1445.600 bt_hci: [BT] BR/EDR 94:53:30:78:B7:AC RSSI -80 CLASS 08043C AUDIO
```
- **The first LE scan** heard 9 devices, and the inquiry 3 within its first 2 s.
- **The BT_INFO lines** come from a diagnostic build of `rtw89_wifi`, not committed.
- **The patched controller's scoreboard** flips bits 31 and 13 (BT_HILNA) hundreds of times a second.

## Related

[700-BLT-0001](700-BLT-0001-bt-hci-the-rtl8821c-up.md) (the RTL8821C's patch), [700-BLT-0003](../issues-done/700-BLT-0003-bt-hci-listens-only-when-asked.done) (listening on request), [213-DRV-0052](213-DRV-0052-coexistence-after-association.md) (the coexistence).
