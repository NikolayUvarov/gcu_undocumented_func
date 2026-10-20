# 550-KRN-0097 — The Wi-Fi station in the boot services: `wlan`, its radio, `keystore`'s client, the stack's third card and `SLOT_WIFI`

**Type:** `init`, ABI (`common/abi.rs`, ABI 10), tests · **Owner:** kernel session · **Priority:** P1 · **Status:** in progress (made on the branch; the `net` suite, the gate and the MacBook Pro's run left) · **Blocked by:** — (on the MacBook Pro, `bcm_wifi` answers as a radio once 550-DRV-0027 is in `main`) · **Main task:** [550](550-network-on-real-hardware.md) · **Constitution:** MC-2.3, MC-3.11, MC-12.4

## Problem

The network track's station service `wlan` (550-NET-0018, in `main` since aedea68) sits between a radio serving `idl/wlanhw.wit` and the network stack. Only `init` can start it and hand out its clients. Three requests wait in `requests-KRN.md`:

- **the network track's "The Wi-Fi station in the boot services"** (550-NET-0018), with `keystore`'s client for remembered networks (550-NET-0024);
- **the drivers track's note** (550-DRV-0027): `bcm_wifi` serves `idl/wlanhw.wit` on `SLOT_SERVICE` when it is given one and brings the radio up with it; `init` gives it the endpoint and lends its client to `wlan` alone;
- **the tools track's "A slot for the Wi-Fi station's configuration client"** (550-APP-0033): `SLOT_WIFI` and `REQUEST_WIFI`, with the shell holding the client that changes settings.

A first version waited for the network track's code (8b102ac, prepared 2026-10-12, not pushed); this one is that version brought to ABI 10, with `bcm_wifi` and `keystore` added.

## Plan

1. **Boot images and order:**
   - `wlansim.elf` (QEMU's stand-in radio, only in the tests' images) and `wlan.elf` are boot images 26 and 27 (counted from 0).
   - `tpm` and `keystore` move before them, and `netstack` after them: `wlan` needs `keystore`'s client when it starts, and `netstack` needs `wlan` running to get its third card.
   - The bootloader skips an image its manifest does not list.
2. **`init`:**
   - `bcm_wifi`: a server endpoint as well (550-DRV-0027).
   - `wlansim`: its endpoint and nothing else.
   - `wlan`, only when a radio runs (`bcm_wifi`, else `wlansim`):
     - its endpoint;
     - the radio's client in slot 2, which no other task gets (MC-2.3);
     - `keystore`'s client badged `BADGE_KEY_STATION` in slot 3.
   - `netstack`: `wlan`'s client without a badge in slot 4, its third card.
   - The shell: `wlan`'s client with `BADGE_WIFI_CHANGE` in `SLOT_WIFI`.
   - A safe start leaves `bcm_wifi` out (000-KRN-0076), so `wlan` then runs only over `wlansim`.
3. **ABI 10:**
   - `SLOT_WIFI` = 33, a launch slot; `SLOT_DYNAMIC` = 34;
   - `BOOT_IMAGES` = 36, in the new order;
   - `REQUEST_WIFI` (`1 << 22`) in `mind::process`;
   - docs/api and docs/profile/bootstrap.md say so.
4. **The `net` suite** puts `wlansim` on its disk where the fixture scripts built it (x86: `scripts/x86_fixtures.sh`; aarch64: `build_aarch64.sh --fixtures`). It checks the grants with `stat caps` (`wifi_boot_check`).

## Acceptance criteria

- [ ] In the `net` suite with `wlansim` on the disk (x86 and aarch64):
  - `wlan` runs and holds the radio's client in slot 2, and `keystore`'s with badge 2 in slot 3;
  - no task but `init` (its keeper) and `wlansim` itself holds an endpoint `wlansim` serves.
- [ ] `stat caps` shows `netstack`'s slot 4, and the shell's `SLOT_WIFI` with badge `0x300`, as clients of `wlan`.
- [ ] Without `wlansim`, `wlan` is not started, and every other suite runs as before.
- [ ] On the MacBook Pro with 550-DRV-0027: `wlan` holds `bcm_wifi`'s client, and `[WLAN] RADIO …` names its address.
- `netstack` finding three cards, and the join, are the network track's half (550-NET-0018). The shell's lending for `REQUEST_WIFI` is the tools track's (550-APP-0033).

## Related

[550-NET-0018](550-NET-0018-the-station-service.md), [550-NET-0024](550-NET-0024-remembered-wi-fi-networks.md), [000-KRN-0109](000-KRN-0109-the-shells-clients-may-badge-for-programs.md) (in the same change), 550-DRV-0027 (`bcm_wifi` as a radio), 550-APP-0033 (`wifi`), [docs/profile/bootstrap.md](../docs/profile/bootstrap.md).
