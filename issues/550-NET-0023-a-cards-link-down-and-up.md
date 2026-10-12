# 550-NET-0023 — A card's link going down and coming back

**Type:** network (service) · **Owner:** `NET` track (worked by the storage session at the maintainer's request) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** track D · **Constitution:** MC-6.1, MC-12.3

The part of the request "Several network interfaces" (requests-NET.md) that the network stack can do with the cards it has: what happens when a card's link goes and comes back. More cards than two, USB Ethernet and the station's card wait for the drivers and for `init`.

## Problem

`netstack` reads a card's link state once, when the card first answers. A cable pulled or a Wi-Fi network lost leaves the card's address and routes in place, so new flows go out through a link that carries nothing, and open connections hang until they time out. The station (`wlan`, 550-NET-0018) answers `info` only while joined, so the stack needs to notice when it stops.

## Plan

- **Watching.** `netstack` asks each card for `info` once a second. A link down, or a card that stops answering, is down.
- **Down:**
  - the card's address, gateway and default route go, so new flows go out through another card;
  - its TCP connections are aborted, so their owners learn at once (`Closed`) instead of waiting;
  - the log says `CARD n LINK DOWN: ADDRESS DROPPED, k CONNECTIONS ABORTED`.
- **Up again:** DHCP starts over (`CARD n LINK UP, WAITING FOR DHCP`). QEMU's static fallback applies only to a card whose link is up.
- **The default route** is the lowest card that is up and has a gateway. Each change is logged: `DEFAULT ROUTE: CARD n`, or `NONE`.
- **Not here:**
  - a user's choice of the default;
  - a card kind (wired before Wi-Fi);
  - `ip` showing the default. These need `socket.wit` and the shell, and wait until the station and a USB card exist.

## Acceptance criteria

In the `net` suite's two-card check (x86, aarch64):
- QMP `set_link n0 off` drops card 0's address and moves the default route to card 1, and a fetch on card 1's network still works;
- `set_link n0 on` gives card 0 a lease again, and the default route comes back to it;
- a fetch through card 0 works again.

## Related

[105](../issues-done/105-multiple-network-cards.done), [550-NET-0018](550-NET-0018-the-station-service.md), [requests-NET.md](requests-NET.md) ("Several network interfaces").
