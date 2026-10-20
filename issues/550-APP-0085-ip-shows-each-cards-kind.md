# 550-APP-0085 — `ip` shows each card's kind and the default card

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open (planned) · **Blocked by:** — (`idl/socket.wit` 2.5 `cards` is in `main`, 550-NET-0025) · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** track D · **Constitution:** MC-12.3

**Recorded by:** the network track (`NET`), 2026-10-12, in `requests-APP.md`, for 550-NET-0025. Numbered by the tools track on 2026-10-20, at the maintainer's word.

## Problem

`netstack` states which card is the default: the card the policy's `default card N` line prefers while its link is up and configured, else the first configured card, wired before the Wi-Fi station. `socket.wit` 2.5 `cards` tells each card's kind (wired or wireless), whether it is there, up and configured, and which is preferred and which is the default. The shell's `ip` shows none of it.

## Plan

- `ip` adds to each card's line its kind (`WIRED`, `WI-FI`) and the words `DEFAULT` and `PREFERRED` where they apply, from `cards`, and lists the station's card even when it is not there yet.
- `ip prefer <card>|none`: a live choice through the operator's `prefer`, until the next start. The lasting choice stays the policy's line (`netpolicy add`).
- The help line and `docs/tools` (EN, RU).

## Acceptance criteria

- **The `net` suite's two-card check:** `ip` shows card 0 as `DEFAULT`; after `netpolicy add default card 1`, card 1 as `PREFERRED DEFAULT`.
- **`ip prefer 1`** makes card 1 the default while it is up, and `ip prefer none` gives it back.

## Related

550-NET-0025 (the default card), [550](550-network-on-real-hardware.md).
