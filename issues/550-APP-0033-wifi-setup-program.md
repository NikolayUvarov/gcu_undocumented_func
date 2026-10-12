# 550-APP-0033 — `wifi`: choosing a Wi-Fi network and typing its passphrase

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open (the program and its screen done against a stand-in, 2026-10-12) · **Blocked by:** the network track's 802.11 station and its configuration interface (`idl/wifi.wit`, proposed by the tools track in [requests-NET.md](requests-NET.md)); a slot for its client ([requests-KRN.md](requests-KRN.md)) · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** track G · **Constitution:** MC-11.9, MC-3.11

_Numbered 550-APP-0017 in the tools branch's commits until main's own 351-APP-0017 (2026-10-09); renumbered at the merge._

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

## Progress (2026-10-12)

The maintainer's word: "do the tools track's part of the network and Wi-Fi configuration". Done on the tools branch, against a stand-in, since neither the station nor its interface exists:

- **`wifi`** ([`wifi/`](../wifi/src)), a program in a `wm` window or on a full screen:
  - [`station.rs`](../wifi/src/station.rs): what it asks of the station (`Station`: scan, join with a passphrase and "remember", status, leave, forget), the passphrase's rules (WPA2-PSK 8 to 63 printable characters or 64 hexadecimal digits; WPA3 1 to 128; WEP and enterprise networks refused with the reason), and `Demo`, a stand-in station with eight fixed access points;
  - [`view.rs`](../wifi/src/view.rs): a row per network name with its strongest access point (signal bars, dBm, channel, protection, "remembered", "joined"), the strongest first and hidden networks last; Enter joins (an open or remembered network at once, else a passphrase shown as dots only, never logged, its buffer overwritten after use); r or F5 scans again, d leaves, f forgets; the link's state on top;
  - `wifi` alone says there is no station yet; `wifi --demo` runs against the stand-in.
- **`wm`:** Settings' network page opens what shows or sets each part of the network: `wifi`, and `ip`, `net`, `netgrants`, `netpolicy` in `console`, which passes them to the shell. The menu lists `wifi` under Network.
- **The interface:** `idl/wifi.wit` 1.0 proposed to `NET` (requests-NET.md), and a slot for its client asked of `KRN` (requests-KRN.md).
- **Tests:** [`tests/wifi_host.rs`](../tests/wifi_host.rs) (5 tests: the rows, the passphrase's rules, joining unseen, a wrong passphrase, the refusals, no station); `tests/wm_host.rs` (the network page's rows); the shell suite runs `wifi` and `wifi --demo` on x86 and aarch64; the `wm` suite opens `wifi` from Settings.
- **Left:** the client of the agreed interface in place of the stand-in, the shell's commands (`wifi list`, `wifi join`), and the acceptance criteria with the station, on the MacBook Pro. These wait for `NET` and `KRN`.

## Related

[550](550-network-on-real-hardware.md), [550-DRV-0006](550-DRV-0006-broadcom-wifi.md), [requests-NET.md](requests-NET.md).
