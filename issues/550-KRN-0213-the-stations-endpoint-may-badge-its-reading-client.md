# 550-KRN-0213 — The station's endpoint may badge its reading client, and a request bit for it

**Type:** `init`, `libmind` (`mind::process`) · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md) · **Constitution:** MC-3.1, MC-3.4

From NET's request of 2026-11-05 for 550-NET-0058 (`issues/requests-KRN.md`, and `KRN/2026-11-05T1545Z-NET-…` on `internal-dev-messages`).

## Problem

**`wlan` keeps two rights apart** (`wifi.wit`):
- a client badged `BADGE_WIFI` reads;
- one badged `BADGE_WIFI_CHANGE` may also join, leave, forget and set the country.

**No program can hold a reading client today.**
- `init` mints `BADGE_WIFI_CHANGE` for the shell and `link`.
- A badge is set once and a copy keeps it (MC-3.4), so the shell cannot narrow its copy.

**`wifi.wit` 1.3's `reader`** (550-NET-0058) has the station mint one `BADGE_WIFI` client from its own endpoint. That needs its endpoint with the badge right, which `init` does not give.

## Plan

1. **`init`:** `wlan`'s endpoint in `SLOT_SERVICE` with `ALL | CAP_BADGE`, as `blockstore`'s (308-KRN-0184). Which badges `wlan` mints is NET's: `BADGE_WIFI` alone, never `BADGE_WIFI_CHANGE` or an unbadged client.
2. **`REQUEST_WIFI_READ`** (`1 << 28`) in `mind::process`: the station's reading client in `SLOT_WIFI`. The shell's lending is APP's (550-APP-0124).
3. **`docs/profile/bootstrap.md`:** the `wlan` row says its endpoint carries the badge right, and for which client.

## Acceptance criteria

- [ ] Builds on x86 and aarch64; the `net` suite passes with the grant.
- [ ] NET's check passes once 550-NET-0058 lands: in the `net` suite over `wlansim`, `wificheck wifi` takes the reading client, reads through it, and is denied the changes.

## Related

550-NET-0058, 550-APP-0124, [550-KRN-0097](../issues-done/550-KRN-0097-the-wifi-station-in-the-boot-services.done), 308-KRN-0184.
