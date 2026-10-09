# 550-APP-0017 — `wifi`: choosing a Wi-Fi network and typing its passphrase

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** the network track's 802.11 station and its configuration interface (`idl/wifi.wit`, proposed; "An 802.11 station and WPA2-PSK" in [requests-NET.md](requests-NET.md)) · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** track G · **Constitution:** MC-11.9, MC-3.11

Numbered from the kernel track's request in `requests-APP.md` (2026-10-08, for 550 at the maintainer's request). That file went once every request in it was numbered.

## Problem

With the 802.11 station that the network track is asked for, the system could join a Wi-Fi network. Nothing lets a person choose one and type its passphrase. The station and its interface do not exist yet, so this waits for them.

## Plan

`wifi` is a program in a `wm` window or on a full screen, and the same as shell commands. It:

- lists the networks found with their name (SSID), signal, security (open, WPA2, WPA3, enterprise) and channel, and refreshes the list;
- connects with a passphrase typed without echo;
- offers to remember the passphrase, and stores it through `keystore`, never in a file (MC-11.9);
- shows the state (connecting; connected, with the address; failed, with the reason);
- forgets a remembered network.

It talks only to the station's Wi-Fi configuration interface, with the badge that may change settings. It never sees frames, or the keys once the passphrase is handed over.

## Acceptance criteria

- **QEMU** (it has no Wi-Fi): the tools suite checks the program against a stand-in station with fixed scan results.
- **The MacBook Pro:** it lists the networks around and joins a WPA2-PSK one.

## Related

[550](550-network-on-real-hardware.md), [550-DRV-0006](550-DRV-0006-broadcom-wifi.md), [requests-NET.md](requests-NET.md).
