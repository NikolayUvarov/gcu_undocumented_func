# 700-BLT-0003 — `bt_hci` listens only when the disk asks: its inquiries left the Ryzen's Wi-Fi deaf

**Type:** driver (`bt_hci`) · **Owner:** `BLT` · **Priority:** P1 · **Status:** open (2026-11-20) · **Blocked by:** — · **Main task:** [700](700-bluetooth.md), [801](801-the-bench-stays-manageable.md) · **Constitution:** MC-6.1, MC-12.1

## Problem

**On the Ryzen, 2026-11-19/20** (`fast-test` 92c2c6e9), the RTL8852BE's Wi-Fi stopped hearing anything for 10 s, and `rtw89_wifi` restarted itself (`RECEIVE STALLED … (801-DRV-0071)`). That happened 3 times in a 30-minute watch, once at boot, and during a release's download, whose TCP stream then broke. Each time the last frame was heard during `bt_hci`'s BR/EDR inquiry (10.24 s, once at start and every 5 minutes), on the same chip's Bluetooth:

| inquiry ends | last frame heard | stall logged |
|---|---|---|
| 22.2 | 21.5 | 31.5 |
| 342.5 | 340.8 | 350.8 |
| 662.8 | 659.9 | 669.9 |
| 2264.3 | 2260.3 | 2270.3 |

Not every inquiry did it: 4 of 8 left the Wi-Fi as it was. The Wi-Fi did not come back by itself after the inquiry ended either. The firmware's coexistence gives the radio to the Bluetooth for the inquiry, and our port sends only the station's first policy (213-DRV-0052), not Linux's TDMA for an inquiry. The rounds exist to show that a controller hears (700-BLT-0002). No user of them exists yet.

## Plan

1. `bt_hci` brings the controller up as before: reset, patch, address. It then listens only when `data/bt-listen.txt` is on the disk, and says which.
2. The coexistence for a Bluetooth inquiry or page, Linux's way, belongs to `rtw89_wifi` (213-DRV-0052). That stays open for when `bt_host` needs inquiries.

## Acceptance criteria

- [ ] On the Ryzen, without the file: `UP: NOT LISTENING`, no inquiry, and no `RECEIVE STALLED` in 30 minutes caused by Bluetooth.
- [ ] With the file, the rounds as before (700-BLT-0002).

## Related

[700-BLT-0002](../issues-done/700-BLT-0002-bt-hci-rounds-every-five-minutes-and-what-they-got.done), [213-DRV-0052](213-DRV-0052-coexistence-after-association.md) (coexistence), [801-DRV-0071](801-DRV-0071-the-bench-radios-keep-their-network-across-releases.md) (the stall's restart).
