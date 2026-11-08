# 550-APP-0124 — The shell lends the Wi-Fi station's reading client, for a program that only shows the link

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — (`REQUEST_WIFI_READ`, `1 << 28`, and `wifi.wit` 1.3's `reader` are in `main` since round 7f260e43, NET's notice of 2026-11-05) · **Main task:** [550](550-network-on-real-hardware.md) · **Constitution:** MC-3.1, MC-6.1

**Asked by:** NET (worked by the storage session), 2026-11-05, in [requests-APP.md](requests-APP.md), for [550-NET-0058](../issues-done/550-NET-0058-a-read-only-wifi-client.done). Numbered the same day.

## Problem

A program that only shows the Wi-Fi link, such as a status line or a widget, can get the station's client only for `REQUEST_WIFI`. That is the client that changes things (`BADGE_WIFI_CHANGE`): it joins, leaves, forgets and sets the country. `wifi.wit` 1.3 (550-NET-0058) adds `reader`, through which the holder of a client asks the station for one that only reads (`BADGE_WIFI`). That client's changing calls answer `denied`.

## Plan

NET's proposal, taken:
- **For `KRN`'s new bit,** the shell asks `wifi::reader(SLOT_WIFI, <a free slot>)`, lends what comes into the program's `SLOT_WIFI`, and drops its own copy.
  - The reading client lets nobody change anything, so starting the program is enough, as for `REQUEST_BATTERY`.
  - In a script, `wifi` must be declared, as now.
- **A program that asks for both bits** gets the changing client, as now.
- **`unsupported`, or no station:** the program runs without the client, as it does without a radio, and the serial line says why.
- **`wm`'s network line and Settings** could take the reading client instead of the shell's changing copy for what they only show. That is a later step of this task, once the bit is there.

## Acceptance criteria

- **Host tests:** which client a start gets, by its bits.
- **QEMU, the `net` suite over `wlansim`:** a program with the bit alone holds a client whose `status` answers and whose `join` answers `denied`; with both bits, `join` works.
- **docs/tools**, English and Russian.

## Related

[550-NET-0058](../issues-done/550-NET-0058-a-read-only-wifi-client.done), [550-APP-0033](550-APP-0033-wifi-setup-program.md) (`wifi`), [requests-KRN.md](requests-KRN.md).
