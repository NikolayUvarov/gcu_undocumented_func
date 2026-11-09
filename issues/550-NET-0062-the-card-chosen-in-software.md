# 550-NET-0062 — The card a machine's traffic takes, chosen in software: the preferred card within its network, and a card out of use

**Type:** network (service, interface) · **Owner:** `NET` track, made by STO by agreement with the track's owner (the kernel session, 2026-11-09) · **Priority:** P1 · **Status:** in progress (2026-11-09) · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** track D · **Constitution:** MC-11.6, MC-12.4, Appendix B.6

Numbered by the network track's owner on 2026-11-09 (`STO/2026-11-09T0744Z-KRN-the-card-chosen-in-software-is-550-net-0062-yours-to-make.md`) from STO's request in `requests-NET.md`, which came from DRV's message of 2026-11-08 for the maintainer.

## Problem

- **The maintainer's word, 2026-11-08:** switching between the channels should be done in software, not by pulling the cable.
- **On the Ryzen** the wired card 0 and the Wi-Fi card 2 had addresses on one network, 192.168.2.0/24. `route()` took the first card whose network held the destination, so the LAN went out through card 0 whatever the policy's `default card` said. Only pulling the cable moved it.
- **What existed** ([550-NET-0025](../issues-done/550-NET-0025-the-default-card.done)): `default card N` moved only the destinations no card's network holds, and nothing took a card out of use.

## Plan

1. **The preferred card within its network.** The routing rule:
   - a card's own address first;
   - then, among the cards whose network holds the destination, the preferred one, then the lowest;
   - then the default card, as before.

   So `default card 2` moves the LAN as well. The stack keeps no host routes.
2. **A card out of use: the policy line `card N off`.** Removing the line brings the card back.
   - **The broker tells the stack** at start, after each change and every 10 s, as it tells the preferred card. It uses `socket.wit` 2.11, a minor version:
     - `withdraw`, for the broker's badge and the operator's. Any other badge is refused, and the log names it;
     - `withdrawn`, which any client may read.
   - **While a card is out of use:**
     - it routes nothing and keeps no address;
     - its TCP connections are aborted, counted in the log, as at a link going down. A listening socket stays;
     - nothing is sent through it, DHCP included;
     - its driver keeps the link, and the stack still asks whether it is up.
   - **Back in use,** DHCP runs on it again.
3. **Every card out of use** is allowed: the operator decides. The log names the risk: Effector reaches the machine over the network, so the link's console (800-APP-0108) is the way back. `netcheck cards` says so.
4. **On a machine without a screen** the consent is the one the policy already asks for. The owner's `link-console` mode lets the link's console run `netpolicy add` and `remove` at once.
5. **`ip` showing it** is the tools track's (a request to APP).

## Acceptance criteria

- **QEMU, the `net` suite, x86 and aarch64,** with two cards on one network:
  - datagrams to an address of that network go out through card 0 by default, and through card 1 after `netpolicy add default card 1`;
  - after `netpolicy add card 0 off`, card 0's address is gone, everything goes through card 1, and card 0's sent frames stop growing;
  - after `netpolicy remove card 0 off`, card 0 gets a lease again.
- **On the Ryzen** (DRV, over the link, with the cable in, after it lands): one command moves all traffic to card 2, LAN and default route alike, and a second brings everything back.

## Related

[550-NET-0025](../issues-done/550-NET-0025-the-default-card.done) (the default card), [550-NET-0023](../issues-done/550-NET-0023-a-cards-link-down-and-up.done) (a link going down), [docs/network/policy.md](../docs/network/policy.md#a-card-out-of-use-550-net-0062).
