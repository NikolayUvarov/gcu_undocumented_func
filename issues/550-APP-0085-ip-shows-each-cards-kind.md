# 550-APP-0085 — `ip` shows each card's kind and the default card

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** in progress (on the tools branch, 2026-10-20; the gate left) · **Blocked by:** — (`idl/socket.wit` 2.5 `cards` is in `main`, 550-NET-0025) · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** track D · **Constitution:** MC-12.3

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

## Progress (2026-10-20)

- **`shell/src/net.rs`, `ip`:** each card's line ends with its kind (`WIRED`, `WI-FI`), then `DOWN`, `PREFERRED` and `DEFAULT` where they apply, from `socket.wit` 2.5 `cards`; a card the stack has a slot for and no driver yet is listed as `CARD 2 WI-FI NOT THERE`. The help line says so.
- **The `net` suite:** the two-card check reads `ip`: `WIRED DEFAULT`, `WIRED`, `WI-FI NOT THERE`; after `netpolicy add default card 1`, card 1 is `WIRED PREFERRED DEFAULT`. After the Wi-Fi join, the station is `CARD 2 … IP 10.0.4.15/24 … WI-FI`.
- **`ip prefer` is not made.** The policy broker tells the stack the policy's card again every 10 s (`PREFER_MS` in `netpolicy/src/main.rs`, docs/network/policy.md), so a live choice through the operator's `prefer` would last 10 s at most. The lasting choice stays the policy's line, `netpolicy add default card N`. The criterion for `ip prefer` is withdrawn for that reason; a live choice would need the broker to keep one, a request to the network track if it is wanted.
- **Left:** the gate on this batch.

## Related

550-NET-0025 (the default card), [550](550-network-on-real-hardware.md).
