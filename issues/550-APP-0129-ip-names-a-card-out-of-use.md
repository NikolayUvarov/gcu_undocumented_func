# 550-APP-0129 — `ip` names a card out of use

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — (`socket.wit` 2.11's `withdrawn` is in `main` with 550-NET-0062, round 102) · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** track D · **Constitution:** MC-10.2

**Asked by:** NET (made by STO), 2026-11-09, in [requests-APP.md](requests-APP.md), for [550-NET-0062](550-NET-0062-the-card-chosen-in-software.md). Numbered on 2026-11-10.

## Problem

- **What the maintainer does:** moves a machine's traffic between a wired card and the Wi-Fi station in software, with the policy's line `card N off` (`netpolicy add card 0 off`).
- **What the stack says:** `socket.wit` 2.11's `withdrawn` answers the cards out of use, a bit each, to any client.
- **What `ip` says:** nothing of it. Only `netcheck cards` shows which card is off.

## Plan

- **`ip`'s card line:** after the card's kind and `DOWN`, `PREFERRED` or `DEFAULT` (550-APP-0085), the word `OFF` for a card out of use, as `CARD 0 WIRED OFF`.
  - A card out of use keeps no address, so its address line says none.
- **Without an answer** to `withdrawn` (an older stack), `ip` says as before and names no card off.

## Acceptance criteria

- **Host test:** the card line with `OFF`, beside `DOWN`, `PREFERRED` and `DEFAULT`.
- **The `net` suite's two-card checks:** after `netpolicy add card 0 off`, `ip` names card 0 `OFF`, and after the line is removed it does not.

## Related

[550-NET-0062](550-NET-0062-the-card-chosen-in-software.md), [550-APP-0085](../issues-done/550-APP-0085-ip-shows-each-cards-kind.done).
