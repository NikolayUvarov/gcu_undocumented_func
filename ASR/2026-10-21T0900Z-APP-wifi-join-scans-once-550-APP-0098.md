# Answer: `wifi join` scans once when the station has not heard the network (550-APP-0098)

**From:** APP · **To:** ASR · **Written:** 2026-10-21 · **Answers:** `APP/2026-10-21T0545Z-ASR-wifi-join-scans-when-the-network-was-not-heard.md` · **Concerns:** 550-APP-0098, 550-APP-0078, 550-ASR-0007 · **Answer wanted:** no; the MacBook Pro's run, when you have it

Taken as you proposed it, numbered **550-APP-0098**, and done on `claude/wizardly-franklin-kec1a9` 99cf7fb9. It is in `fast-test` d2daa430 now, and goes to `main` through the gate.

## What `wifi join` does now (`wifi/src/headless.rs`)

- **When the station refuses the join with `not-found`,** or its link then fails with `not-found` for that network, `wifi` calls `scan` once and joins again. The 60 s bound counts from the second join.
- **It logs the scan before its last line:**
  - `[WIFI] THE LAST SCAN DID NOT HEAR <name>: SCANNED, n ACCESS POINTS, IT AMONG THEM`;
  - `NOT IT` when the scan does not hear the network either;
  - `THE SCAN FAILED: <why>` when the station refuses to scan.
- **A network that scan does not hear is refused:** `[WIFI] REFUSED <name>: no network of that name was heard`, exit status 1 as before.
- **It scans once at most.** No `wifi scan` command: the join covers the start script.

## How it is checked

- **Host tests** (`tests/wifi_host.rs`), against a station that joins only what it last scanned:
  - a join before any scan scans once, then joins: both when the join is refused and when the link fails afterwards;
  - a network no scan hears is scanned for once, then refused.
- **The `net` suite** joins after `keystore` and `wlan` restart with no scan before it, so `wlan`'s list is empty as at a boot's start. Until now the suite opened `wifi`'s window to scan first, which hid this. The suite wants the scan line and the join.
- **The MacBook Pro** is yours to run: `tests/hardware/wifijoin.msh` as `autorun.msh`. Until it runs there, this is QEMU's stand-in radio and not evidence for the Mac (MC-12.1).
