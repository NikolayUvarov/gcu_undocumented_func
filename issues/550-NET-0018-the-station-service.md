# 550-NET-0018 — The station service `wlan` and its configuration interface

**Type:** network (service, interface) · **Owner:** `NET` track (worked by the storage session at the maintainer's request) · **Priority:** P1 · **Status:** open (the QEMU checks pass; the MacBook Pro joined WPA2 and leased) · **Blocked by:** the maintainer's run of `https` over the Mac's Wi-Fi; a client with `BADGE_WIFI` alone, which no program holds yet · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** track D · **Constitution:** MC-2.3, MC-11.9, MC-12.4, Appendix B.6

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

[550-NET-0012](../issues-done/550-NET-0012-the-radio-interface.done) (`idl/wlanhw.wit`), [550-DRV-0027](550-DRV-0027-bcm4331-frame-service.md), [550-APP-0033](550-APP-0033-wifi-setup-program.md), [550-NET-0024](550-NET-0024-remembered-wi-fi-networks.md) (remembered networks).

## Progress (2026-10-20): the MacBook Pro

From the maintainer's run of 2026-10-20 on the MacBook Pro (`fast-test` 832c9080, recorded in 550-ASR-0007 and 550-DRV-0027 by `f750a8af`):
- `wlan` joined `CoolNet2` (WPA2-PSK) on channel 8 through `bcm_wifi`, joined by hand from `wm`'s Settings, Wi-Fi: `[WLAN] JOINING CoolNet2 ON CHANNEL 8` at 256.782 s, `Joined` at 256.823 s;
- the stack leased 192.168.2.236/24 on the station's card 3.6 s later, and the default route went to it;
- `bcm_wifi` logged no transmit fault, and the passphrase is nowhere in the log.

So the station joins a WPA2-PSK network on the Mac. **Left of this criterion:** the shell's `https` fetching a page over that link, which the run did not do. The scripted run (`tests/hardware/wifijoin.msh`, 550-ASR-0007) pings the gateway; an `https` line there would cover it.

## Progress (2026-10-20)

The `net` suite's QEMU checks run with `wlansim`, since `init` starts `wlan` (550-KRN-0097) and `wifi` joins without a screen (550-APP-0078). They passed on x86 and aarch64 in the network track's gate 20 (473b1816, 929d1fa5):
- **Scan and join:** `wifi` lists `MIND-TEST`, and the station joins it with the passphrase (APP's checks).
- **A wrong passphrase** is refused with the station's reason.
- **The lease and the ping:** the stack leases 10.0.4.15 on the station's card, `ip` shows it, and `wifiping` reaches the gateway 10.0.4.1 through the WPA2 link.

Left:
- the check that a client with `BADGE_WIFI` alone is `denied` a join: no program holds such a client yet;
- the MacBook Pro's run, with `bcm_wifi`.

## Progress (2026-10-12)

- **Done:**
  - `idl/wifi.wit` 1.0 (c2ee9ea), told to the tools track in requests-APP.md;
  - [`wlan/`](../wlan/src/main.rs), the service, built for both targets (in `02_build.sh`'s list, so in the images, though nothing starts it yet);
  - [`tests/wlansim/`](../tests/wlansim/src/main.rs), the stand-in radio with its access point and DHCP, ARP and ICMP echo, built with the test programs on both architectures.
  - `netstack` reads slot 4 as its third card (`CARDS` = 3), where 550-KRN-0097 lends it the station's client; the slot is empty without a station, as slot 3 is without a second card;
  - `wlansim`'s network is 10.0.4.0/24, so that it never shares one with the second card's 10.0.3.0/24 in the `net` suite.
- **Not tested yet:** `init` does not start `wlan` or `wlansim` on `main`. The kernel track prepared that as 550-KRN-0097, which reaches `main` after this task's code. The QEMU checks of the criteria come with it.
