# 550-APP-0104 — The `wifi` guide names the station's own join

**Type:** tools (docs) · **Owner:** tools track (`APP`) · **Priority:** P3 · **Status:** open · **Blocked by:** [550-NET-0041](550-NET-0041-wlan-joins-a-remembered-network-by-itself.md) in `main` (its code is on `claude/youthful-mendel-mf1soy` 3199a4e3, and in `fast-test`) · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** track H · **Constitution:** MC-12.3

**Recorded by:** the tools track, 2026-10-22, from the network track's request (worked by the kernel session) in `requests-APP.md` on that branch, for 550-NET-0041.

## Problem

- **What changes in `wlan`.** With 550-NET-0041, the station joins a remembered network by itself, at start and after the network is lost ([docs/network/station.md](../docs/network/station.md)).
- **What the guide says.** `docs/tools` describes `d` (`wifi leave`) and `f` (forget) without that. A user who leaves a network may not expect the station to join it again by itself.

## Plan

- **One line in the `wifi` guide,** beside `d` and `f` (`docs/tools/README.md` and `README_RU.md`):
  - the station joins a remembered network by itself;
  - `d` stops that until the station next joins;
  - `f` takes the network out of it.
- **Not before the code is in `main`.** Until then the line would state a behaviour `main` does not have (MC-12.3).
- **Later, if the network track adds it to `wifi.wit`:** a word in `wifi` to turn the station's own join off for good.

## Acceptance criteria

- The line is in both guides once 550-NET-0041's code is in `main`.
- The request is taken from `requests-APP.md`.

## Related

- [550-NET-0041](550-NET-0041-wlan-joins-a-remembered-network-by-itself.md); [550-APP-0078](550-APP-0078-wifi-join-without-a-screen.md) (`wifi leave` without a screen).
