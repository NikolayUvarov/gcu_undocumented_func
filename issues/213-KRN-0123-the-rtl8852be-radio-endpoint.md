# 213-KRN-0123 — The RTL8852BE as a radio of `wlan`: its `wlanhw` endpoint after the driver's datapath is admitted

**Type:** `init` · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** the drivers track's 213-DRV-0032 (a persistent driver loop and admitted datapath); [213-KRN-0114](213-KRN-0114-realtek-driver-startup.md) (the Realtek drivers started with DMA off, in `main` since 289e84b); [213-KRN-0122](213-KRN-0122-platform-power-policy-for-the-wlan-device.md) before transmission · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-2.3, MC-3.3, MC-6.1, MC-12.1, MC-12.3

## Problem

213-KRN-0114 (from the `ASTRA` branch, in `main` since 289e84b) starts `rtw89_wifi` as a diagnostic: `init` lends it BAR2, retained DMA and a firmware reader, but no `SLOT_SERVICE`, and `init`'s list of radios holds only `bcm_wifi` and `wlansim` ([550-KRN-0097](../issues-done/550-KRN-0097-the-wifi-station-in-the-boot-services.done)). So the driver cannot become a station's radio. The request of 2026-10-16 is in [requests-KRN.md](requests-KRN.md); both target machines are in Uzbekistan, and `UZ` is this deployment's explicit first choice, not a default for other installations.

## Plan

1. **A server endpoint** in `SLOT_SERVICE` for the selected RTL8852BE, and its client lent to `wlan` only, as for `bcm_wifi`. The MacBook Pro's BCM4331 and QEMU's `wlansim` keep their selection and authority; a running radio is not replaced because a second PCI device exists.
2. **The driver's slots 9 and 10** stay empty for its deferred requests, or the drivers track changes the contract first. The log, BAR, DMA and `vfs` grants stay as 0114 has them.
3. **A bounded country and channel configuration** for the radio, apart from 0122's platform policy: missing or invalid input is an unavailable radio, never a default country. Its interface follows the network track's country selection (`requests-NET.md`) and the tools track's selector (`requests-APP.md`); this task carries it to the driver, it does not define the catalogue.
4. **Only when admitted.** The endpoint is given when the driver says its datapath is admitted; a running diagnostic is not readiness. At first one admitted 2.4 GHz / 20 MHz channel, other channels refused.
5. **Restart** keeps retained DMA and the verified non-mastering restart of 0114; no grant here carries platform authority.

## Acceptance criteria

- [ ] Fixtures or QEMU cases: no Realtek device; a missing or invalid configuration; the driver refusing to start; exactly one `wlanhw` client, held by `wlan`; the BCM4331 and `wlansim` routes unchanged.
- [ ] On the Intel MacBook Pro and the Ryzen, each separately: association, DHCP and ping under test load, recorded per machine, before a physical claim is made.

## Related

[213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md), [213-KRN-0114](213-KRN-0114-realtek-driver-startup.md), [213-KRN-0122](213-KRN-0122-platform-power-policy-for-the-wlan-device.md), [550-KRN-0097](../issues-done/550-KRN-0097-the-wifi-station-in-the-boot-services.done), [requests-NET.md](requests-NET.md), [requests-APP.md](requests-APP.md).
