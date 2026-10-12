# 550-NET-0018 — The station service `wlan` and its configuration interface

**Type:** network (service, interface) · **Owner:** `NET` track (worked by the storage session at the maintainer's request) · **Priority:** P1 · **Status:** open · **Blocked by:** — for the interface and the service; `KRN` for starting it and lending its clients ([requests-KRN.md](requests-KRN.md), "The Wi-Fi station in the boot services") · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** track D · **Constitution:** MC-2.3, MC-11.9, MC-12.4, Appendix B.6

The third part of the request in requests-NET.md ("An 802.11 station and WPA2-PSK"), after [550-NET-0016](../issues-done/550-NET-0016-wpa2-psk-library.done) and [550-NET-0017](../issues-done/550-NET-0017-the-stations-logic.done).

## Problem

The station's logic (`dot11`) does no I/O. Something has to drive it:
- **The radio.** It is a client of a radio that serves `idl/wlanhw.wit` (`bcm_wifi`, 550-DRV-0027).
- **The network stack.** It is a card for `netstack` (`idl/net.wit`).
- **The configuration.** `wifi`, the program the tools track built against a stand-in (550-APP-0033), needs an interface to the station.

## Plan

- **[`idl/wifi.wit`](../idl/wifi.wit) 1.0:** scan, join, status, leave and forget. It was agreed from the tools track's proposal (requests-NET.md) with changes the tools track is told of (requests-APP.md):
  - SSIDs as bytes;
  - no address, which is the network stack's;
  - `other`, `not-found`, `scanning`;
  - a scan that answers when it ends.
- **The badges** (`mind::network`): `BADGE_WIFI` reads and `BADGE_WIFI_CHANGE` also changes. The station's one endpoint serves `net.wit` to a client without `BADGE_WIFI` (the stack), so a program with the configuration client never reaches the frames.
- **`wlan`, a boot service:**
  - it holds the radio's client and its own endpoint, and nothing else that can act;
  - it drives `dot11` from the radio's frames (`receive`, `wait`) and a clock;
  - it serves `net.wit` 1.3 to the stack (`send`, `receive`, `wait`, `info`, `counters`; no ring, no offload) and `wifi.wit` to clients with `BADGE_WIFI`;
  - a passphrase becomes a PMK at once and is not kept.
- **Remembering a network** needs `keystore` to keep a PMK under a name (a minor version of `idl/keystore.wit`, the network track's). Until then `join` with `remember` answers `unsupported`.
- **QEMU has no Wi-Fi device.** A stand-in radio (`wlansim`, a test service) serves `wlanhw.wit` as a radio within reach of one WPA2-PSK access point. That access point is simulated as in `dot11`'s tests, with `wpa::authenticator`, and answers DHCP, ARP and ICMP echo itself. Through it the stack gets a lease over the station and `ping` answers.
- **`netstack`:** a third card, in the slot `KRN` gives it.

## Acceptance criteria

- `idl/wifi.wit` is generated and fuzzed with the others (`tests/idl_fuzz_host.rs`).
- In QEMU (the `net` suite, x86 and aarch64), with `wlansim` and once `KRN` starts `wlan` and lends its clients:
  - the shell's client scans and lists the simulated network as WPA2;
  - it joins with the passphrase;
  - the stack gets a lease on the station's card, and `ping` answers through it;
  - a wrong passphrase gives `wrong-passphrase`;
  - a client with `BADGE_WIFI` alone is `denied` a join.
- On the MacBook Pro, with `bcm_wifi` serving `wlanhw.wit`: the station joins a WPA2-PSK network and the shell's `https` fetches a page (the request's criterion).

## Related

[550-NET-0012](../issues-done/550-NET-0012-the-radio-interface.done) (`idl/wlanhw.wit`), [550-DRV-0027](550-DRV-0027-bcm4331-frame-service.md), [550-APP-0033](550-APP-0033-wifi-setup-program.md).

## Progress (2026-10-12)

- **Done:**
  - `idl/wifi.wit` 1.0 (c2ee9ea), told to the tools track in requests-APP.md;
  - [`wlan/`](../wlan/src/main.rs), the service, built for both targets (in `02_build.sh`'s list, so in the images, though nothing starts it yet);
  - [`tests/wlansim/`](../tests/wlansim/src/main.rs), the stand-in radio with its access point and DHCP, ARP and ICMP echo, built with the test programs on both architectures.
- **Not tested yet:** `init` does not start `wlan` or `wlansim`, and the stack has no third card (requests-KRN.md). The QEMU checks of the criteria come with that.
