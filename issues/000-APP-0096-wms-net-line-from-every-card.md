# 000-APP-0096 — `wm`'s network line counts every card, the Wi-Fi station's too

**Type:** tools (`wm`, `shell`, interface) · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** in progress (on the tools branch, 2026-10-20; the gate and the MacBook Pro left) · **Blocked by:** — · **Main task:** — (split from [000-APP-0049](000-APP-0049-network-traffic-on-wms-background.md)) · **Roadmap:** track D, G · **Constitution:** MC-3.11, MC-12.3, MC-12.4

**Recorded by:** the tools track, 2026-10-20, from the maintainer's report: "now we have network working but desktop widget with date time cpu load says 'no network counters'".

## Problem

- **`wm`'s `show = net` line reads one card** (000-APP-0049): the client of the first wired card's driver that `init` gives the shell in `SLOT_NET`, lent as a copy with `BADGE_COUNTERS`.
- **Over Wi-Fi it has nothing to read.** The station's card is the stack's third (`wlan`), and no client of it reaches the shell.
  - With no wired card (the MacBook Pro), the shell holds no `SLOT_NET` and lends nothing. `wm` says `NET: NO COUNTERS` while the network works.
  - With a wired card that carries nothing, it would show that card's traffic and not the Wi-Fi's.
- **The line does not say why.** `NET: NO COUNTERS` is all it shows, whether there is no card, no stack or no client.

## Plan

- **`idl/shell.wit` 1.7 `net-counters`:** every card's frames as `netstack` counts them (`socket.wit` `interfaces`), with its kind and whether it is the default (`cards`).
  - The shell answers it from its operator client of the stack.
  - It is for the window manager the shell started; `refused` for any other client.
  - It reads only and reaches nothing on the network (MC-3.11: the authority stays the shell's).
- **`wm` sums every card's frames** for `NET IN n/S OUT n/S`.
  - The shell's answer comes first. Without it, `wm` reads the card client lent with `BADGE_COUNTERS`, as before.
  - The shell is asked with a 50 ms bound. When it does not answer in time (busy with a long command), it is left alone for 10 s and the line stays as it was.
- **The line says why it has nothing:**
  - `NET: NO CARD`: the stack counts no card;
  - `NET: NO NETWORK STACK`;
  - `NET: NO COUNTERS`: neither source.
- **The first rate in `wm`'s log** names its source and the cards counted: `(EVERY CARD'S FRAMES, 2 CARDS, 1 WI-FI, THE FIRST RATE)`.

## Acceptance criteria

- **The `net` suite** (x86, aarch64): `wm` logs `[WM] NET IN … (EVERY CARD'S FRAMES, n CARDS, w WI-FI, THE FIRST RATE)`, with `1 WI-FI` when the stand-in radio runs (`wlansim`).
- **The MacBook Pro over Wi-Fi:** the line moves while a download runs.

## Progress (2026-10-20)

- **`idl/shell.wit` 1.7** and `libmind/src/idl/shell.rs` (generated).
- **`shell/src/net.rs` `card_counters`;** the shell's `net-counters` answer for the window managers it started.
- **`wm/src/main.rs` `net_frames`:** the shell first, the card client second, the reason otherwise, the 10 s pause after a slow answer.
- **The `net` suite** reads the first rate's line with the cards counted.
- **Docs:** `docs/tools` (EN, RU), `docs/idl`.
- **Left:** the gate; the MacBook Pro's run.

## Related

[000-APP-0049](000-APP-0049-network-traffic-on-wms-background.md) (the line, the card client), [000-NET-0015](000-NET-0015-a-card-read-only-client.md) (`BADGE_COUNTERS`), [550-NET-0018](550-NET-0018-the-station-service.md) (the station's card).
