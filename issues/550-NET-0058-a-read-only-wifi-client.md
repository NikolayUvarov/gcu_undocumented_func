# 550-NET-0058 — A read-only Wi-Fi client, and its refusal of changes checked

**Type:** network (`wlan`), with `init`'s grant and the shell's lending · **Owner:** `NET` · **Priority:** P3 · **Status:** in progress (2026-11-05) · **Blocked by:** `init`'s grant of the station's endpoint with the badge right ([requests-KRN.md](requests-KRN.md)) · **Main task:** [550](550-network-on-real-hardware.md) · **Constitution:** MC-6.1, MC-12.2

**Split from:** [550-NET-0035](../issues-done/550-NET-0035-the-stations-country.done), whose criterion "a read-only client is refused" could not be tested.

## Problem

- **`wlan` keeps two rights apart.** A client badged `BADGE_WIFI` reads: it scans, and reads the status, the countries and the country. One badged `BADGE_WIFI_CHANGE` may also join, leave, forget and set the country (`may_change` in `wlan/src/main.rs`).
- **No program holds a reading client:**
  - `init` lends `wlan` only with `BADGE_WIFI_CHANGE`, to the shell and to `link`;
  - the shell lends that same client to a program that asks for Wi-Fi (`REQUEST_WIFI`). Its slot has no right to badge, so it cannot make a narrower copy.
- **So the refusal is untested,** and a program that only shows the network gets the right to change it (MC-6.1).

## Plan

1. **A reading client for programs** that only look, such as a status line. The shell cannot narrow its own copy: a badge is set once and a copy keeps it (`kernel/src/scheduler.rs`, MC-3.4). An unbadged client with the badge right would widen the shell instead: without `BADGE_WIFI`, `wlan` takes a client for the network stack's card (`net.wit`). So the station mints the reading client itself:
   - **`wifi.wit` 1.3 `reader`** (`NET`): `wlan` mints one `BADGE_WIFI` client from its own endpoint at its start and answers it to any client of the interface; `unsupported` when its endpoint has no right to badge.
   - **`init`'s grant** (`KRN`): `wlan`'s endpoint with `ALL | CAP_BADGE`, as `blockstore`'s (308-KRN-0184), and a request bit for a program that asks for the reading client alone. Recorded in [requests-KRN.md](requests-KRN.md).
   - **The shell's lending** (`APP`): for that bit, it asks `reader` and lends what it gets. Recorded in [requests-APP.md](requests-APP.md).
2. **`wlan`'s answers to it:** every changing call (`join`, `leave`, `forget`, `set-country`) is `denied`, and nothing changes.
3. **A QEMU check** in the `net` suite, with a program that holds the reading client: `netcheck wifi`, lent the changing client (`REQUEST_WIFI`), asks `reader`, drops what it was lent and works through what it got.

## Acceptance criteria

- **QEMU, x86 and aarch64:** a program with the reading client reads the status and the country; `set-country` and `join` from it are `denied`, and the station's country and link are as before.
- **Docs:** `docs/network` and `docs/profile/bootstrap.md` name the client and who holds it.

## Progress

- **2026-11-05, on `claude/relaxed-meitner-5bmhpz` at eca06c5c3:**
  - `idl/wifi.wit` 1.3 `reader` and its generated `libmind/src/idl/wifi.rs`;
  - `wlan` minting the reading client;
  - `netcheck wifi`, and `wificheck wifi` in the `net` suite's joined-station check;
  - `docs/network/station.md` 0.2, and the `wifi` rows of `docs/idl/README.md` and `docs/profile/bootstrap.md`.
- **Tested in this session's container (4 CPUs),** with the branch on `main` at 27cebd982 and `init`'s line from the KRN request applied but not committed:
  - the `net` suite passed on x86 (35 checks) and on aarch64 (32), with `wificheck wifi` among them;
  - the host tests passed.
- **Not done yet:**
  - `init`'s grant (`KRN`). Without it `reader` answers `unsupported` and the new check fails, so the landing request waits for it.
  - The request bit (`KRN`) and the shell's lending (`APP`): a program that does not first hold the changing client cannot get a reading one yet.

## Related

[550-NET-0035](../issues-done/550-NET-0035-the-stations-country.done), [550-KRN-0097](../issues-done/550-KRN-0097-the-wifi-station-in-the-boot-services.done) (the station in the boot services), 000-NET-0015 (the shell's read-only copies of other clients).
