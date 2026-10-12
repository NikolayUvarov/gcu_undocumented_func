# 550-APP-0033 — `wifi`: choosing a Wi-Fi network and typing its passphrase

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open (the program follows the agreed `idl/wifi.wit` 1.0, against a stand-in, 2026-10-12) · **Blocked by:** a slot for the station's client ([requests-KRN.md](requests-KRN.md)); the station running on the MacBook Pro (550-NET-0018, 550-DRV-0027) · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** track G · **Constitution:** MC-11.9, MC-3.11

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

## Progress: `idl/wifi.wit` 1.0 (2026-10-12)

The network track agreed the interface (550-NET-0018) with changes it asked of this track in requests-APP.md ("`wifi` follows `idl/wifi.wit` 1.0 as agreed", on the network branch until it reaches `main`). `wifi` follows them:

- **The interface in this branch:** `idl/wifi.wit` and its generated `libmind/src/idl/wifi.rs` as the network track committed them (c2ee9ea), so that the same files merge cleanly; the IDL index row and the fuzzer's entry stay the network track's.
- **[`station.rs`](../wifi/src/station.rs):**
  - an SSID is bytes, shown as UTF-8 where it is and as `\xNN` where it is not (control characters too), so a name stays exact;
  - the signal is minus dBm (0 unknown, listed last), the access point a BSSID, the rate in 500 kb/s units;
  - `security.other`, `error.not-found`, `reason.not-found` and `unsupported`, `state.scanning`; a lost link that the station joins again;
  - no address: the link shows the access point, signal, channel and rate, and says the address is the network stack's (`ip`);
  - the station joins open and WPA2 networks only: WEP, WPA3, enterprise and other protections are refused with the reason before a passphrase is asked;
  - `Demo` serves 1.0: ten access points (a Latin-1 name, a TKIP and a WPA3 network among them), `not-found`, `unsupported`, `invalid`; `remembers` false answers `remember` with `unsupported`, as the station does until it keeps keys.
- **[`view.rs`](../wifi/src/view.rs):** a `remember` the station answers with `unsupported` is asked again without, and the line says the key was not kept; `forget` answered so says the station does not remember networks yet. The passphrase is no longer copied: the line it was typed in is overwritten after the call.
- **[`service.rs`](../wifi/src/service.rs):** `Station` over the station's client with `mind::idl::wifi`. It builds and is not run: no launcher lends the client yet (requests-KRN.md, "A slot for the Wi-Fi station's configuration client"), so `wifi` without `--demo` says so.
- **Tests:** [`tests/wifi_host.rs`](../tests/wifi_host.rs), 8 tests (SSIDs as bytes, the stand-in's answers, a station that does not remember); the shell suite's `wifi --demo` check names the access point joined. What is tested is the program against the stand-in, not the station (MC-12.2).

## Related

[550](550-network-on-real-hardware.md), [550-DRV-0006](550-DRV-0006-broadcom-wifi.md), [requests-NET.md](requests-NET.md).
