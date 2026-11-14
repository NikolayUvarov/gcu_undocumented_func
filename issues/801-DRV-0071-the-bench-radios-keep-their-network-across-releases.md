# 801-DRV-0071 — The bench's radios keep their network across releases

**Type:** driver (`rtw88_wifi`, `rtw89_wifi`, `bcm_wifi`) · **Owner:** `DRV` · **Priority:** P1 · **Status:** open (2026-11-14) · **Blocked by:** — (the platform policy's side is `KRN`'s, 213-KRN-0122) · **Main task:** [801](801-the-bench-stays-manageable.md) · **Constitution:** MC-6.4, MC-12.1

## Problem

**The DEXP lost its Wi-Fi on 2026-11-14** with release 202611140747. `rtw88_wifi` served, but `init` said the platform's power policy was `UNKNOWN (WRDS-in-DSDT)`, so no radio sends.

**The Ryzen's RTL8852BE** lost its network 55 s after joining, with no line from the driver (2026-11-13 at 1245 s, `main` 20fba2c5 plus 550-DRV-0069). This is 213-DRV-0032's silent stall.

## Plan

1. **The DEXP:** with `KRN`, the DSDT's WRDS read on that machine (dumped from it), so its `rtw88_wifi` sends under its limits as before.
2. **The Ryzen:**
   - the data mode's silent stall made visible: a line, and the data mode restarted, not left dead;
   - a radio restart taken up by `wlan` (550-DRV-0069, in `fast-test`).
3. **Each bench radio, across one release:** joined before, joined after, or the trial fails (S1).

## Acceptance criteria

- **Joined:** the DEXP, the Ryzen and the MacBook Pro join CoolNet2 on the same release.
- **The Ryzen:** a 30-minute run with no silent loss, or each loss logged and recovered by itself.

## Related

[801](801-the-bench-stays-manageable.md), [213-KRN-0122](213-KRN-0122-platform-power-policy-for-the-wlan-device.md), [213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md), 550-DRV-0069 (on its branch, in `fast-test`).
