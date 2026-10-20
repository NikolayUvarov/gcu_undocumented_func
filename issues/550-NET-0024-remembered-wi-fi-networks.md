# 550-NET-0024 — Remembered Wi-Fi networks, kept by `keystore`

**Type:** network (service, interface) · **Owner:** `NET` track (worked by the storage session at the maintainer's request) · **Priority:** P1 · **Status:** open · **Blocked by:** — for the code; `KRN` for `keystore`'s client in `wlan` ([requests-KRN.md](requests-KRN.md), "The Wi-Fi station in the boot services") · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** track D · **Constitution:** MC-11.9, MC-12.4, Appendix B.6

From the request "An 802.11 station and WPA2-PSK" in requests-NET.md: "The passphrase or PMK of a remembered network is kept by `keystore`, which could derive each association's keys for the station without handing out the PMK (MC-11.9)". `idl/wifi.wit` 1.0 has `remember` and `forget`; [550-NET-0018](550-NET-0018-the-station-service.md)'s `wlan` answered `unsupported` and remembered nothing.

## Problem

A network's PMK opens all its traffic, other stations' too, for anyone who records it. The station needs it at every join and rejoin, but under MC-11.9 access to key material and the right to use it are distinct. If `wlan` kept the PMKs, a flaw in its frame parsing would hand them out. `wlan` also has no storage of its own.

## Plan

- **`wpa`:**
  - `handshake::Pairwise`: where a handshake's PTK comes from, either a PMK the station holds or a key service. The supplicant asks it once per new message 1, and stops (`NoKey`) when it gives nothing.
  - `kept`: the file of remembered networks. Up to 16, each name and PMK wrapped together (RFC 3394, AES-256) under a key the caller gives, so a name cannot be moved onto another network's PMK.
- **`dot11`:**
  - a join takes `Secret::Pmk` or `Secret::Kept`; `Kept` takes each handshake's keys from the caller's `Pairwise`;
  - a key service that gives nothing ends the join (`refused`);
  - once the keys are installed, a handshake frame in the clear is dropped, so a forged message 1 cannot spend the key service's budget.
- **`idl/keystore.wit` 1.1** (appended):
  - `wifi-remember`, `wifi-networks`, `wifi-keys` (KCK, KEK and TK of one handshake) and `wifi-forget`;
  - only for the station's badge `BADGE_KEY_STATION` (`mind::network`);
  - no call returns a PMK.
- **`keystore`:**
  - keeps the file in its private directory, wrapped under a key derived from the device key, so it is sealed by the TPM where the device key is (351-NET-0006);
  - answers at most 60 handshakes' keys a minute;
  - logs every answer and refusal.
- **`wlan`:**
  - `remember` hands the PMK to `keystore` and drops its own copy, then joins with `Secret::Kept`;
  - an empty passphrase joins a remembered network;
  - `scan` marks remembered networks, and `forget` forgets through `keystore`;
  - without `keystore`'s client, `remember` and `forget` answer `unsupported`.
- **`KRN`'s part:** `init` lends `wlan` a client of `keystore` with `BADGE_KEY_STATION`, in slot 3 (requests-KRN.md).

## Acceptance criteria

- **Host tests** (`scripts/host_tests.sh`):
  - `wpa`: a handshake whose keys come from a key service, which is asked once for a repeated message 1, and one stopped by a service that gives none;
  - `wpa`: the file of remembered networks, which:
    - keeps a name and a PMK, replacing one by name, and stops at 16;
    - shows no name or PMK as it is;
    - opens only under its key, and refuses a changed byte, a cut file and two records of one name;
  - `dot11`: a join with `Secret::Kept`, data under the keys the service gave, a forged handshake frame in the clear dropped without asking it, and a rejoin it refuses;
  - the IDL fuzzer with `keystore.wit` 1.1.
- **In QEMU** (`net` suite, x86 and aarch64), once `KRN` lends the clients:
  - a join with `remember` to the stand-in's `MIND-TEST` is remembered;
  - after a restart of `wlan` and `keystore` (or a reboot), a join with no passphrase gets a lease;
  - `keystore`'s log shows each handshake's keys for the station's PID;
  - `forget` forgets it, and a join with no passphrase is then `invalid`;
  - a client of `keystore` without the badge is `denied` the Wi-Fi calls.
- **On the MacBook Pro:** a remembered network is joined again after a reboot (with 550-NET-0018's criterion).

## Related

[550-NET-0016](../issues-done/550-NET-0016-wpa2-psk-library.done), [550-NET-0017](../issues-done/550-NET-0017-the-stations-logic.done), [550-NET-0018](550-NET-0018-the-station-service.md), [351-NET-0005](../issues-done/351-NET-0005-persistent-device-key.done), [351-NET-0006](351-NET-0006-device-key-sealed-by-a-tpm.md), [550-APP-0033](550-APP-0033-wifi-setup-program.md).

## Progress (2026-10-20, later): a restart and `forget`

The end of the `net` suite's `wifi_join_check` (x86 and aarch64) now also checks:
- **a restart:** after `keystore` and then `wlan` are killed and restarted by `init`, `wifi` scans, and `wifi join MIND-TEST` with no passphrase joins; `keystore` logs the handshake's keys for the new station's PID, and `wifiping` reaches the gateway 10.0.4.1 through the link;
- **`forget`** through `wifi`'s screen (`f`): `wifi` shows `MIND-TEST forgotten: its key is gone from the station`, `keystore` logs `WI-FI NETWORK MIND-TEST FORGOTTEN FOR PID`, and a join with no passphrase is then refused (`the station does not take that passphrase`, the station's `invalid`).

Run locally (x86, QEMU 8.2, TCG, 4 CPUs): it passed. The gate is to come.

Left:
- **a client of `keystore` without the badge `denied`:** the programs `init` lends a `keystore` client are `wlan` (the station's badge), `tls` and `ssh` (their own badges), all services; no program a test controls holds one. The check needs a test fixture holding such a client, which `init` would have to lend (the kernel track's).
- **the MacBook Pro.**

## Progress (2026-10-20)

The `net` suite (x86 and aarch64, gate 20) checks:
- a join with `--remember`, which `keystore` keeps (`WI-FI NETWORK MIND-TEST KEPT FOR PID`);
- a join with no passphrase after `wifi leave`, whose handshake keys come from `keystore` (`WI-FI KEYS FOR MIND-TEST FOR PID`).

Left:
- the join after a restart of `wlan` and `keystore`, or a reboot;
- `forget` (no `wifi forget` without a screen yet);
- a client without the badge `denied`;
- the MacBook Pro.

## Progress (2026-10-12)

- **Built,** as planned: `wpa` (`Pairwise`, `kept`), `dot11` (`Secret`), `idl/keystore.wit` 1.1, `keystore`'s [`wifi.rs`](../keystore/src/wifi.rs) and `wlan`. All build for `x86_64-unknown-none` and `aarch64-unknown-none-softfloat`.
- **The host tests pass:** `wpa` 11 tests (2 new), `dot11` 10 tests (1 new), the IDL fuzzer.
- **Not tested yet:** the QEMU checks, which need `KRN` to lend the clients.
- `docs/profile/threat-model.md` states what the remembered networks are exposed to.
