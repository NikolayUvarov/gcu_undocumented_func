# 550-NET-0058 — A read-only Wi-Fi client, and its refusal of changes checked

**Type:** network (`wlan`), with `init`'s grant and the shell's lending · **Owner:** `NET` · **Priority:** P3 · **Status:** open · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md) · **Constitution:** MC-6.1, MC-12.2

**Split from:** [550-NET-0035](550-NET-0035-the-stations-country.md), whose criterion "a read-only client is refused" could not be tested.

## Problem

- **`wlan` keeps two rights apart.** A client badged `BADGE_WIFI` reads: it scans, and reads the status, the countries and the country. One badged `BADGE_WIFI_CHANGE` may also join, leave, forget and set the country (`may_change` in `wlan/src/main.rs`).
- **No program holds a reading client:**
  - `init` lends `wlan` only with `BADGE_WIFI_CHANGE`, to the shell and to `link`;
  - the shell lends that same client to a program that asks for Wi-Fi (`REQUEST_WIFI`). Its slot has no right to badge, so it cannot make a narrower copy.
- **So the refusal is untested,** and a program that only shows the network gets the right to change it (MC-6.1).

## Plan

1. **A reading client for programs** that only look, such as a status line: a request of its own beside `REQUEST_WIFI`, or the shell's `SLOT_WIFI` with the right to badge, so that it lends a `BADGE_WIFI` copy. The request bits and `init`'s grants are `KRN`'s, the shell's lending `APP`'s: each change there is a task of that track.
2. **`wlan`'s answers to it:** every changing call (`join`, `leave`, `forget`, `set-country`) is `denied`, and nothing changes.
3. **A QEMU check** in the `net` suite, with a program that holds the reading client.

## Acceptance criteria

- **QEMU, x86 and aarch64:** a program with the reading client reads the status and the country; `set-country` and `join` from it are `denied`, and the station's country and link are as before.
- **Docs:** `docs/network` and `docs/profile/bootstrap.md` name the client and who holds it.

## Related

[550-NET-0035](550-NET-0035-the-stations-country.md), [550-KRN-0097](../issues-done/550-KRN-0097-the-wifi-station-in-the-boot-services.done) (the station in the boot services), 000-NET-0015 (the shell's read-only copies of other clients).
