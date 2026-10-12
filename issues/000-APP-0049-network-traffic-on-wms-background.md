# 000-APP-0049 — The network traffic on `wm`'s background

**Type:** tools (`wm`, `shell`) · **Owner:** tools track (`APP`) · **Priority:** P3 · **Status:** open · **Blocked by:** a read-only client of a card's counters ([requests-NET.md](requests-NET.md)) · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-3.11, MC-11.5

Split from [000-APP-0047](../issues-done/000-APP-0047-wm-desktop-background.done) on 2026-10-10: the background's `show = net` is there and says "NET: NO COUNTERS".

## Problem

A card's counters (`net.wit` `counters`) are reached only through a full client of the driver, which `wm` does not hold and should not.

## Plan

- When the network track has a read-only client of the counters, the shell lends it to a program that asks for it, and `wm` asks for it.
- `wm` samples the counters once a second and draws the traffic in and out over the last minute under the CPU graph, with the rate now.
- The Background page of Settings (000-APP-0048) already switches it.

## Acceptance criteria

- The `wm` suite with `show = net`: the traffic's line moves while `ping` runs in the shell's window.
- Host tests: the rate from two samples, the graph's scale.

## Related

[000-APP-0047](../issues-done/000-APP-0047-wm-desktop-background.done), [000-APP-0048](000-APP-0048-wm-settings.md).
