# 550-APP-0098 — `wifi join` scans once when the station has not heard the network

**Type:** tools (`wifi`) · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** track G · **Constitution:** MC-12.1, MC-12.3

**Recorded by:** the tools track, 2026-10-21, from the assurance track's request on `internal-dev-messages` (`APP/2026-10-21T0545Z-ASR-wifi-join-scans-when-the-network-was-not-heard.md`), for [550-ASR-0007](550-ASR-0007-wifi-join-tests.md) and [550-APP-0078](550-APP-0078-wifi-join-without-a-screen.md).

## Problem

- **The MacBook Pro's start script did not join.** Its `autorun.msh` (`tests/hardware/wifijoin.msh`) runs `wifi join` for a network `keystore` remembers. `wlan` answered `NOT FOUND` and `wifi` ended with 1: `no network of that name was heard` (`fast-test` ffd5dcc).
- **The station joins only from its last scan** (`dot11` `Station::join`, `idl/wifi.wit` `join`), and `wlan` scans only when asked. Nothing had scanned since the boot.
  - The `wifi` window scans when it opens, so a join by hand works.
  - `wifi join` without a screen never scanned, so a script could not join at start.
- **The `net` suite hid it.** In its VM whose radio answers late, the station's only scan is made while the radio is silent, as at the Mac's start; the suite opened `wifi`'s window to scan again before `wifi join`.

## Plan

- **`wifi/src/headless.rs` `join`:** when the station refuses the join with `not-found`, or its link fails with `not-found` for that network, `wifi` scans once and asks again.
  - It logs the scan before its last line: `[WIFI] THE LAST SCAN DID NOT HEAR <name>: SCANNED, n ACCESS POINTS, IT AMONG THEM` (or `NOT IT`, or `THE SCAN FAILED: <why>`).
  - A network that scan does not hear either is refused: `[WIFI] REFUSED <name>: no network of that name was heard`. The window's text, which names its key `r`, is not used here.
  - It scans once at most, and the 60 s bound counts from the last join.
- **Docs:** `docs/tools` (EN, RU).

## Acceptance criteria

- **Host tests** (`tests/wifi_host.rs`), against a station that joins only what it last scanned:
  - a join before any scan scans once, then joins, both when the join is refused and when the link fails afterwards;
  - a network no scan hears is scanned for once, then refused.
- **The `net` suite** (x86, where the slow radio is built): in the VM whose radio answers late, `wifi join MIND-TEST` with no scan after the radio answered logs the scan line with `IT AMONG THEM` and joins.
  - Not after `keystore` and `wlan` restart, as first tried (gate if6b6 on i9, 2026-10-21). There the restarted station joined without a scan: `dot11` takes every beacon it hears into its list, and `wlansim`, not restarted, still passes the joined network's beacons.
- **The MacBook Pro:** `tests/hardware/wifijoin.msh` as `autorun.msh` joins the remembered network at start. This is QEMU's stand-in radio until then (MC-12.1).

## Related

[550-APP-0078](550-APP-0078-wifi-join-without-a-screen.md) (`wifi join` without a screen), [550-ASR-0007](550-ASR-0007-wifi-join-tests.md) (the hardware tests), [000-APP-0079](000-APP-0079-the-shells-start-script.md) (the start script), [550-NET-0018](550-NET-0018-the-station-service.md) (the station).
