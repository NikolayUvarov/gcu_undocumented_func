# 550-APP-0033 — `wifi`: choosing a Wi-Fi network and typing its passphrase

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open: the station's client is lent to `wifi`, from the shell and from `wm`'s Settings, in `main` since 92781301 (2026-10-20); the MacBook Pro's check left · **Blocked by:** the MacBook Pro's check · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** track G · **Constitution:** MC-11.9, MC-3.11

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
- **[`service.rs`](../wifi/src/service.rs):** `Station` over the station's client with `mind::idl::wifi`. It builds and is not run: no launcher lends the client yet (requests-KRN.md, "A slot for the Wi-Fi station's configuration client"), so `wifi` without `--demo` says so. The kernel track has prepared it (550-KRN-0097, requests-NET.md): the shell's client with `BADGE_WIFI_CHANGE` in `SLOT_WIFI` (33) and `REQUEST_WIFI`, reaching `main` with 550-NET-0018. Then `wifi` asks for it and uses `Service`, and `wm` passes it on.
- **Tests:** [`tests/wifi_host.rs`](../tests/wifi_host.rs), 8 tests (SSIDs as bytes, the stand-in's answers, a station that does not remember); the shell suite's `wifi --demo` check names the access point joined. What is tested is the program against the stand-in, not the station (MC-12.2).

## Progress: the station's client (2026-10-14)

- **The kernel's side** (550-KRN-0097, ABI 10, on `claude/youthful-mendel-mf1soy` and in `fast-test`): `init` gives the shell `wlan`'s configuration client, with `BADGE_WIFI_CHANGE`, in `SLOT_WIFI` (33) while a radio runs. A program asks for it with `REQUEST_WIFI`.
- **The shell** lends its `SLOT_WIFI` to a program that asks for it: starting `wifi` is the user's request to choose a network. A script declares it as `requires: wifi`.
- **`shell.wit` 1.5** adds `wifi`: a copy of the shell's client for the window manager the shell started with its commands; any other client is `refused`, and without a station it is `unavailable`. `wm` asks for it at its start, as for the battery's (`[WM] WI-FI CLIENT FROM THE SHELL: …`), and lends it to `wifi` from Settings, Network.
- **`wifi`** asks for `REQUEST_WIFI` and, holding the client, scans and joins through `wlan` instead of the stand-in. Without it, it says how to start it.
- **Tests:**
  - the net suite, whose disk has QEMU's stand-in radio (`wlansim`, 550-NET-0018), lists `MIND-TEST`, joins it through `wlan`'s WPA2 handshake with the passphrase typed, and leaves it;
  - the shell and `wm` suites, with no radio, still see no station (`[WM] WI-FI CLIENT FROM THE SHELL: NONE (NO STATION)`).
- **The join in QEMU** (2026-10-14):
  - **The first run** (PCU, c874f13) listed `MIND-TEST`, but the join ended at once with `LINK=OFF`. The suite's last x86 VM ran QEMU's `qemu64` CPU, which has no RDRAND, and `wlan` refuses WPA2 without it (issue 103).
  - **The fix is in the test:** that VM now gets `-cpu qemu64,+rdrand`, as the TLS VM has (2683f51). `wifi` also logs the line under its list as `[WIFI] NOTICE …` (df50186), so a failed join says why.
  - **On 2683f51** the net group passed on `un-rizen-service` (`x86: boot, display, network, TLS, shell, memory, clock`, 864 s). `wifi` listed `MIND-TEST`, joined it through `wlan`'s WPA2 handshake with `wlansim`'s access point, and left it.
  - This is QEMU's stand-in radio, not the MacBook Pro's (MC-12.1). The MacBook Pro 10,1's Ivy Bridge CPU has RDRAND; a run without it would log `[WLAN] NO RDRAND: WPA2 REFUSED`.
- **Where:** the task branch `claude/550-APP-0033-wifi-station-client` (the tools branch with the kernel track's ABI 10 merged) and `fast-test`. It reaches `main` after the kernel track's ABI 10 does, through this track's gate.
- **In `main`** since 92781301 (2026-10-20), after the kernel track's ABI 10 (now 11) reached it: the gate on 92781301 passed (every x86 group on i9, every aarch64 group on un-rizen-service, the rest locally).
- **Left:** the maintainer's check on the MacBook Pro (`wifi` lists the networks around, and joins one).

## Related

[550](550-network-on-real-hardware.md), [550-DRV-0006](550-DRV-0006-broadcom-wifi.md), [requests-NET.md](requests-NET.md).
