# 000-APP-0049 — The network traffic on `wm`'s background

**Type:** tools (`wm`, `shell`) · **Owner:** tools track (`APP`) · **Priority:** P3 · **Status:** in progress (on the tools branch, 2026-10-20; the gate left) · **Blocked by:** — (`REQUEST_NET_COUNTERS` and `SLOT_NET` with `CAP_BADGE`: ABI 10, 000-KRN-0109; the client: [000-NET-0015](000-NET-0015-a-card-read-only-client.md), `net.wit` 1.3) · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-3.11, MC-11.5

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

## Progress (2026-10-20)

- **The shell** lends a copy of its `SLOT_NET` client minted with `BADGE_COUNTERS` (only `info`, `counters`, `offloads`; the drivers refuse the rest, 000-NET-0015) in the program's `SLOT_NET`, for `REQUEST_NET_COUNTERS`; a script's program gets it with `network` declared. `wm` asks for it, and passes it on to the programs it starts that ask.
- **`wm`** reads the counters once a second while `show = net`: `background::net_line` gives `NET IN <n>/S OUT <n>/S` (frames a second; `K` from 10 000), and the line replaces `NET: NO COUNTERS`. It logs its first rate, since the background's pixels are not text for a suite.
- It is the first wired card's: the shell's `SLOT_NET` holds one card. A graph over the last minute is not drawn: the line only.
- **Host test:** `tests/wm_host.rs`, the rate from two samples, `K`, a driver that counts from 0 again.
- **The `net` suite** (x86, aarch64, virtio-net): with `data/wm.conf` saying `show = net`, `wm` logs `[WM] NET IN … (THE CARD'S COUNTERS, THE FIRST RATE)` after `ping`. Its `wm` suite has no card, so the check is there.
- **Left:** the gate on this batch; the line moving while `ping` runs is not checked in QEMU.

## Related

[000-APP-0047](../issues-done/000-APP-0047-wm-desktop-background.done), [000-APP-0048](000-APP-0048-wm-settings.md).
