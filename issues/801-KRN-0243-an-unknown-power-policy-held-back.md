# 801-KRN-0243 — An unknown power policy held back: `init`'s line says what it does, and no radio is silenced for it

**Type:** kernel (`init`) · **Owner:** `KRN` · **Priority:** P1 · **Status:** open (2026-11-14) · **Blocked by:** — (`rtw89_wifi`'s side: [requests-DRV.md](requests-DRV.md) 1) · **Main task:** [801](801-the-bench-stays-manageable.md) (S5) · **Roadmap:** stage V · **Constitution:** MC-12.3, MC-12.1

## Problem

**`init`'s line was not true.** It logged `[INIT] PLATFORM POWER POLICY FOR WLAN: UNKNOWN (<why>): A RADIO SENDS NOTHING` (213-KRN-0122). On the DEXP that line was read as the cause of its lost Wi-Fi (801, 801-DRV-0071), but it was not:
- **`init` gives the platform word only to `rtw89_wifi`.** `rtw88_wifi`, the DEXP's radio, is given the PCI index alone and reads no word.
- **The DEXP's networks were lost elsewhere.** The trial's `keystore` held another device key, so the remembered networks did not open (801-NET-0079).

**`rtw89_wifi` does take `Unknown` as no transmission.** The maintainer (801 S5): a rule that would silence a radio that worked is held back, logged and not enforced.

## Plan

- **`init`'s line says what it does:** `… UNKNOWN (<why>): HELD BACK, NOT ENFORCED; ONLY rtw89_wifi IS GIVEN THE WORD`. `libmind/src/platform_power.rs` says the same.
- **The word given to `rtw89_wifi` stays `platform=unknown:<why>`,** which is the truth. Sending under it is the driver's part, asked in [requests-DRV.md](requests-DRV.md) 1: within the country's regulatory limits and its calibration, as before 213-KRN-0122.
- **`rtw88_wifi` and `bcm_wifi` are given no word,** as before.

## Acceptance criteria

- **The `normal` suite under U-Boot** (aarch64, a device tree, no DSDT) finds the new line.
- **That group** passes on the maintainer's machines, then in a gate.
- **DRV's side** is a task of its own; it closes the radio's half.

## Related

[801](801-the-bench-stays-manageable.md), [213-KRN-0122](213-KRN-0122-platform-power-policy-for-the-wlan-device.md), [801-DRV-0071](801-DRV-0071-the-bench-radios-keep-their-network-across-releases.md), [requests-DRV.md](requests-DRV.md).
