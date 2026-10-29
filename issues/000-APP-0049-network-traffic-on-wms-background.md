# 000-APP-0049 — The network traffic on `wm`'s background

**Type:** tools (`wm`, `shell`) · **Owner:** tools track (`APP`) · **Priority:** P3 · **Status:** open: in `main` since 835c31c8 (2026-10-20); the line moving while `ping` runs in the shell's window is checked (2026-10-29); the graph over the last minute is on the tools branch (2026-10-29) · **Blocked by:** — (`REQUEST_NET_COUNTERS` and `SLOT_NET` with `CAP_BADGE`: ABI 10, 000-KRN-0109; the client: [000-NET-0015](../issues-done/000-NET-0015-a-card-read-only-client.done), `net.wit` 1.3) · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-3.11, MC-11.5

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
- **In `main`** since 835c31c8 (2026-10-20). The gate: every x86 group on i9 and every aarch64 group on un-rizen-service (Effector); the host tests, TLC and the x86 groups boot, services, scheduling, files and keys locally. The batch's later commits changed only the suites, and each group ran on a commit that has the suites it runs.
- **Over Wi-Fi the card client has nothing to read** (the maintainer's report, 2026-10-20: `NET: NO COUNTERS` while the network works): the line now comes from every card the stack counts, through the shell, in [000-APP-0096](000-APP-0096-wms-net-line-from-every-card.md); the card client stays its second source.
- **Left:** the line moving while `ping` runs is not checked in QEMU; a graph over the last minute.

## Progress (2026-10-29): the line moving while `ping` runs

- **The check** (the `net` suite, as the `wm` suite has no card):
  - wm starts with `show = net` and logs its first rate;
  - the shell's window opens (Ctrl+Alt+F5), and `ping -c 5 10.0.2.2` is typed into it;
  - wm logs `NET IN 1/S OUT 1/S (THE LINE MOVED)` while it runs, and `NET IN 0/S OUT 0/S (THE LINE STILL)` after it.
- **The log of the line's moves** (`background::NetMoves`): the moves between still and traffic, 16 at most, as the background's pixels are not text. Host-tested in `tests/wm_host.rs`.
- **Two faults the check found** (in `main` since 000-APP-0096):
  - **The line froze for good** once a call for the counters had timed out. Each tick of the quiet time that followed started it again, so the shell was never asked again. The shell's window opening made that first call time out. Now only a call that timed out starts the quiet time.
  - **No sample while the shell is busy.** A command running in the shell, such as `ping`, stops it answering, and the sample after it spread its frames over the whole wait: five over six seconds made 0/S. The card client the shell lends (`BADGE_COUNTERS`) now stands in while the shell is quiet. It reaches the driver itself, and each source keeps its own last sample. The first rate in the log is still the shell's when the shell gives one.
- **Passed:** the x86 `net` suite under QEMU TCG with 4 CPUs, in this session's container. The aarch64 run is recorded with the landing request.
- **Left:** the graph over the last minute, and its scale's host test.

## Progress (2026-10-29): the graph over the last minute

- **What it draws** (`wm/src/background.rs`, the digits' face): under the NET line, a graph as wide as the CPU's, a bar for each of the last 60 samples, one a second, newest at the right. The frames received go up from its middle line and the frames sent go down from it.
- **Its scale** (`net_scale`): the minute's largest rate either way, rounded up to 1, 2 or 5 times a power of ten, 1 frame a second at least. A line under the graph names it: `↑IN ↓OUT 0–5/S, 1 MIN`.
- **Its samples** (`net_rates`): thousandths of frames a second between two samples of a source, so a ping a second over samples a little more than a second apart still shows. `wm` keeps a minute of them (`Backdrop::traffic_sample`) and drops them when it has no counters.
- **The analog face** keeps the line alone.
- **Host test:** `tests/wm_host.rs`, `the_net_graph_shows_the_last_minute_both_ways`. It covers the rates from two samples, the scale's steps, its line, and the bars: full and half heights both ways, the middle line, and nothing drawn outside the block.
- **Left:** the landing in `main`; then this issue closes.

## Related

[000-APP-0047](../issues-done/000-APP-0047-wm-desktop-background.done), [000-APP-0048](000-APP-0048-wm-settings.md).
