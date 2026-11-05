# 213-KRN-0123 — The RTL8852BE as a radio of `wlan`: its `wlanhw` endpoint once the driver serves it, transmission once its datapath is admitted

**Type:** `init` · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** the drivers track's 213-DRV-0032 for transmission (its datapath admitted); [213-KRN-0122](213-KRN-0122-platform-power-policy-for-the-wlan-device.md) before transmission · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-2.3, MC-3.3, MC-6.1, MC-12.1, MC-12.3

## Problem

213-KRN-0114 (from the `ASTRA` branch, in `main` since 289e84b) starts `rtw89_wifi` as a diagnostic: `init` lends it BAR2, retained DMA and a firmware reader, but no `SLOT_SERVICE`, and `init`'s list of radios holds only `bcm_wifi` and `wlansim` ([550-KRN-0097](../issues-done/550-KRN-0097-the-wifi-station-in-the-boot-services.done)). So the driver cannot become a station's radio. The request of 2026-10-16 is in [requests-KRN.md](requests-KRN.md); both target machines are in Uzbekistan, and `UZ` is this deployment's explicit first choice, not a default for other installations.

## Plan

1. **A server endpoint** in `SLOT_SERVICE` for the selected RTL8852BE, and its client lent to `wlan` only, as for `bcm_wifi`. The MacBook Pro's BCM4331 and QEMU's `wlansim` keep their selection and authority; a running radio is not replaced because a second PCI device exists.
2. **The driver's slots 9 and 10** stay empty for its deferred requests, or the drivers track changes the contract first. The log, BAR, DMA and `vfs` grants stay as 0114 has them.
3. **The country comes from the station, not from `init`** (the maintainer, 2026-11-04: "different countries, not only one").
   - Any country of the regulatory catalogue (wireless-regdb, 181 and `00`), as 550-NET-0035 plans: the user chooses it (`wifi country XX`, `wm`'s Settings, 550-APP-0082), `keystore` keeps it, and `wlan` sends the radio its limits with `wlanhw.wit` 1.2 `set-country`.
   - No country is a default: until one is applied the radio sends nothing. `UZ` is set once on the maintainer's machines, not in the code.
   - This task gives `rtw89_wifi` the `wlanhw` endpoint that call reaches. The driver answers `set-country` (DRV's part). `init` carries no country.
   - Only 213-KRN-0122's platform result reaches the driver from `init`.
4. **The endpoint before admission, transmission after it** (revised 2026-11-05 with `DRV`, `KRN/2026-11-05T0555Z-DRV-…`). Tying the endpoint to admission would deadlock: admission needs a calibrated channel, calibration needs the country's rule, and the country comes from `wlan` through this endpoint.
   - **The endpoint is given to a driver that serves `wlanhw`.** It logs `[RTW89] SERVING WLANHW: RECEIVE ONLY UNTIL A COUNTRY IS APPLIED` once its service loop runs. Until then it answers `info` and `set-country`, delivers received frames, and refuses every `send` with `failed`.
   - **Transmission is the driver's to refuse** until three things hold: a country applied, 213-KRN-0122's word not `unknown`, and the channel's calibration passed. Then it logs `[RTW89] DATAPATH ADMITTED ON CHANNEL n`. A later country without that channel takes it back to receive only, with a line saying so.
   - **`init` does not read a driver's log.** It grants the endpoint at the driver's start, as for `rtw88_wifi` and `bcm_wifi`, and puts `rtw89_wifi` among the radios `wlan` may drive. So the gate is the order of landing: this part lands with `DRV`'s service loop or after it, never before. A driver that serves no endpoint would leave `wlan` calling a radio that never answers.
   - **The endpoint carries no transmit authority,** so a running diagnostic is still not readiness. At first one admitted 2.4 GHz / 20 MHz channel; other channels are refused.
5. **Restart** keeps retained DMA and the verified non-mastering restart of 0114; no grant here carries platform authority.

## Acceptance criteria

- [ ] Fixtures or QEMU cases: no Realtek device; no country applied (nothing sent); the driver refusing to start; exactly one `wlanhw` client, held by `wlan`; the BCM4331 and `wlansim` routes unchanged.
- [ ] On the Ryzen: `SERVING WLANHW`, then `wlan`'s `set-country`, then `DATAPATH ADMITTED ON CHANNEL n` only after the country, the word `absent` and calibration; no frame sent before it.
- [ ] On the Intel MacBook Pro and the Ryzen, each separately: association, DHCP and ping under test load, recorded per machine, before a physical claim is made.

## Progress

2026-11-05, `init`'s part, on `claude/213-KRN-0123-endpoint`:
- `rtw89_wifi` gets its endpoint in `SLOT_SERVICE` at its start, with the grants 0114 and 0122 gave it.
- `rtw89_wifi` is among `RADIOS`, after `rtw88_wifi` and before `wlansim`; `wlan` takes the first that runs.
- Docs: `docs/profile/bootstrap.md` (`rtw89_wifi` and `wlan`), `docs/profile/network.md`, `README.md`.

Tested on 525982245, through Effector (the boots named that commit), all passed:
- acc-1, x86: `boot, display, shell, memory, clock` (no Realtek or Broadcom driver started: `NOT STARTED: NO DEVICE`); `network, TLS and Effector` (`wlan` over `wlansim`'s client, which nobody else holds);
- acc-2, aarch64: `boot and fault containment`, `network, TLS and Effector`.

QEMU has no RTL8852BE, so these show only that the other routes are unchanged (MC-12.1).

It lands with `DRV`'s service loop or after it, never before (point 4): it waits for that landing request.

2026-11-05, landed in round 86 (`main` 6253e7a7), after `DRV`'s service loop (`claude/ASR-DRV` 5849050b, `[RTW89] SERVING WLANHW: RECEIVE ONLY UNTIL A COUNTRY IS APPLIED`) in the same round. [213-KRN-0114](213-KRN-0114-realtek-driver-startup.md) is in `main` since 289e84b.

**Left:** the criteria, on the Ryzen and the MacBook Pro once `DRV` admits the datapath.

## Related

[213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md), [213-KRN-0114](213-KRN-0114-realtek-driver-startup.md), [213-KRN-0122](213-KRN-0122-platform-power-policy-for-the-wlan-device.md), [550-KRN-0097](../issues-done/550-KRN-0097-the-wifi-station-in-the-boot-services.done), [requests-NET.md](requests-NET.md), [requests-APP.md](requests-APP.md).
