# 550-NET-0025 — The default card by a stated rule, and the policy's preferred card

**Type:** network (service, interface) · **Owner:** `NET` track (worked by the storage session at the maintainer's request) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** track D · **Constitution:** MC-11.6, MC-12.4, Appendix B.6

From the request "Several network interfaces" in requests-NET.md: "The default interface follows a stated rule (for example, a wired link that is up before Wi-Fi), and the user can override it. `ip` shows the default and the kind of each card."

## Problem

`netstack` takes as its default the first card with a gateway, whatever its kind. With the Wi-Fi station as a third card ([550-NET-0018](550-NET-0018-the-station-service.md)), which card a machine's traffic leaves through then follows the order of `init`'s slots. Nothing says so, nobody can choose otherwise, and no client can tell a card's kind.

## Plan

- **The rule, stated** (`idl/socket.wit` 2.5). A flow goes out through the card whose network holds its destination. Otherwise it goes through the default card:
  - the preferred card, while its link is up and it has an address and a gateway;
  - else the first card that has, wired cards (0 and 1, slots 2 and 3) before the Wi-Fi station (2, slot 4);
  - else any card with an address.
- **`prefer(card)`** names the preferred card, 255 for none. Only the operator and policy badges may call it; a grant gets `denied`.
- **`cards()`** gives each card's number and kind (wired or wireless): whether its driver answered, whether its link is up and it is configured, and whether it is the preferred card and the default one.
- **The user's choice is a policy line**, `default card N`, changed with `netpolicy add` and `remove`, which ask the user first (108):
  - the broker checks it (`N` below 3);
  - it tells the stack at start, after each change and every 10 s, so a restarted stack learns it again;
  - every change is logged.
- **`netcheck cards`** prints the cards, and `netcheck sockets` gives each socket's card.
- **`ip`** showing each card's kind and the default is the shell's, so it goes to the tools track (requests-APP.md).

## Acceptance criteria

In the `net` suite's two-card check (x86, aarch64):
- `netcheck cards` shows both wired cards present, up and configured, card 0 the default, the station's card 2 wireless and absent, none preferred;
- datagrams to an address on neither card's network leave through card 0;
- after `netpolicy add default card 1`:
  - card 1 is preferred and the default, and the datagrams leave through it;
  - with card 1's link down they leave through card 0, and through card 1 again once it is back;
- `default card 3` is refused;
- after `netpolicy remove default card 1`, card 0 is the default again;
- the IDL fuzzer runs with `socket.wit` 2.5.

## Related

[550](550-network-on-real-hardware.md), [550-NET-0023](550-NET-0023-a-cards-link-down-and-up.md), [550-NET-0018](550-NET-0018-the-station-service.md), [108](../issues-done/108-editable-network-policy.done), [docs/network/policy.md](../docs/network/policy.md).
