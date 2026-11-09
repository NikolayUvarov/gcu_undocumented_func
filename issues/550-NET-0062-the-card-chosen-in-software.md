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

## Progress (2026-11-09)

**Built** on `claude/relaxed-meitner-5bmhpz`, by STO for the network track:
- **`netstack`, the routing rule** (`route()`): a card's own address; then the cards whose network holds the destination, the preferred one first, then the lowest; then the default card, as before.
- **`netstack`, a card out of use** (`withdraw`, `take_out`; `Link::off`):
  - the card's address and default route are dropped, and its TCP connections are aborted and counted. Listening sockets (`Kind::Listen`) stay;
  - its interface is not polled, so nothing is sent, DHCP included. `watch_link` still asks its driver whether the link is up;
  - a card that appears while out of use starts so;
  - back in use, DHCP starts over;
  - every card out of use is allowed, and logged with the way back (the link's console);
  - a `withdraw` from any badge but the policy's and the operator's is refused and logged with its badge and PID.
- **`netpolicy`:** the line `card N off`, checked as any line (`card 3 off` is `Invalid`), and the mask told to the stack with the preferred card: at start, after each change and every 10 s.
- **`idl/socket.wit` 2.11** (minor): `withdraw` and `withdrawn`, with the regenerated `libmind/src/idl/socket.rs`.
- **`netcheck`:** `cards` adds `OFF` and `NETCHECK EVERY CARD OUT OF USE`; `withdraw:MASK` asks as a grant, which is refused.
- **Docs:** `docs/network/policy.md` 0.8 ("A card out of use") and `docs/idl`.
- **`ip`:** asked of APP in `requests-APP.md`.

**Tested** in this session's container (4 CPUs, QEMU 8.2.2 under TCG), on aa26af05, built with the x86 and aarch64 fixtures:
- **The `net` suite:** x86 38 checks, aarch64 35, all passed. aarch64 skips 3, the same as before this change: fixtures built for x86 only.
- **Its new check** (`one_network_cards_check`), two cards on one QEMU network, both leased 10.0.2.15/24:
  - six datagrams to the gateway go out through card 0, through card 1 with `default card 1`, and through card 0 again without that line;
  - with `card 0 off`, card 0's address goes, the default route moves to card 1, and `netcheck cards` shows `OFF`. A grant's `withdraw` is `Denied` and logged with its badge. Card 0's sent frames do not grow over 12 s, past the broker's next telling;
  - with `card 1 off` too, the stack logs every card out of use and `netcheck` says so. Card 1 is then put back and leased again;
  - with `card 0 off` removed, card 0 is leased again and takes the network's flows back.
- **The stack's other users:** on the same images, at 51db3f24, `tls`, `effector` and the `updater` group passed on x86 (23 checks, including `update`'s HTTPS fetch and `updatestore`), and `tls` and `effector` on aarch64 (17).
- **`scripts/host_tests.sh`:** every Rust test passed. One Python test failed: EFF's `effector_server_test.py` `test_contract_tls_pin_auth_and_prefix`, the failure reported to EFF on 2026-11-05, which fails the same way on `main` in this container.
- Not run on the Ryzen: DRV's check comes after this lands.

**Not done:**
- **DRV's check on the Ryzen.**
- **`ip` naming a card out of use** (APP).
- **A card's own address first** is not shown in QEMU: there both cards hold 10.0.2.15, so it takes card 0.

## Related

[550-NET-0025](../issues-done/550-NET-0025-the-default-card.done) (the default card), [550-NET-0023](../issues-done/550-NET-0023-a-cards-link-down-and-up.done) (a link going down), [docs/network/policy.md](../docs/network/policy.md#a-card-out-of-use-550-net-0062).
