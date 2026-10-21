# 550-APP-0098 — `wifi join` scans once when the station has not heard the network

**Type:** tools (`wifi`) · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** open: in `main` since 1d841622 (2026-10-21), checked in QEMU; the MacBook Pro's run left · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** track G · **Constitution:** MC-12.1, MC-12.3

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

## Progress (2026-10-21)

- **`wifi/src/headless.rs` `join`**, its host tests (`tests/wifi_host.rs`, `a_join_before_any_scan_scans_once_then_joins` and the network no scan hears), and the `net` suite's late-radio VM, done.
- **In `main`** with landing round 2, `main` 1d841622 (2026-10-21): the kernel session's gate of candidate 97c4a4c1. On the branch before it: every group of `ci_local.sh --list` on fe83e221 (i9, un-rizen-service and locally; five x86 groups passed again alone after a load of 53 on i9, and `files and block writes` after the parser sessions' race of 109-KRN-0133), and the `net` and `wm` groups locally on 893ef487. The late radio joined `scanning by itself first` on i9 and locally.
- **ASR's answer:** `ASR/2026-10-21T0900Z-APP-wifi-join-scans-once-550-APP-0098.md` on `internal-dev-messages`.
- **Left:** the MacBook Pro's start script joins at start (MC-12.1: QEMU's stand-in radio is not the Mac's).

## Related

[550-APP-0078](550-APP-0078-wifi-join-without-a-screen.md) (`wifi join` without a screen), [550-ASR-0007](550-ASR-0007-wifi-join-tests.md) (the hardware tests), [000-APP-0079](000-APP-0079-the-shells-start-script.md) (the start script), [550-NET-0018](550-NET-0018-the-station-service.md) (the station).
