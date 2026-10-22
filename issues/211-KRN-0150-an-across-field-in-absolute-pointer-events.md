# 211-KRN-0150 — Horizontal wheel steps in absolute pointer events

**Type:** ABI (`pointer_absolute_scroll`, ABI 15) · **Owner:** kernel session · **Priority:** P3 · **Status:** open (2026-10-22): taken into ABI 15 with 800-KRN-0148, the code next · **Blocked by:** — · **Main task:** — (for [211-APP-0090](211-APP-0090-horizontal-scrolling.md)) · **Constitution:** MC-12.4, MC-12.7

From the tools track's request in [requests-KRN.md](requests-KRN.md) (2026-10-20), numbered here and taken into ABI 15 with [800-KRN-0148](800-KRN-0148-a-shell-client-for-link-in-developer-mode.md).

## Problem

- **A relative pointer event** carries the horizontal wheel in bits 34–37 (`pointer_scroll`, `pointer_across`).
- **An absolute event** keeps its vertical wheel in those bits, and has no field for the horizontal one.
- **A window's queue gets only absolute events** (`mind::window::pointer_at`). So `wm` cannot pass a swipe left or right to the window under the pointer, and a tablet in QEMU cannot send one.

## Plan

- **Where the steps go.** An absolute event's byte field (bits 0–7, zero in every pointer event) carries the horizontal steps in bits 0–3: signed, positive to the right.
- **`pointer_absolute_scroll(buttons, x, y, wheel, across)`** makes such an event. `pointer_absolute` and `pointer_absolute_fields` stay as they are, so their callers do not change.
- **`pointer_across(event)`** reads the steps of either kind of event. Before, it read 0 for an absolute one. An event made before the change still reads 0.
- **An ABI change,** in ABI 15.

## Acceptance criteria

- [ ] A host test round-trips an absolute event with across steps from −8 to 7, with its position, buttons and wheel unchanged. An event from `pointer_absolute` reads 0.
- [ ] The gate passes. `wm` and `mind::window::pointer_at` passing the steps on are the tools track's (211-APP-0090).

## Related

[211-APP-0090](211-APP-0090-horizontal-scrolling.md), 211-DRV-0018 (the trackpad's swipes), [800-KRN-0148](800-KRN-0148-a-shell-client-for-link-in-developer-mode.md) (ABI 15).
