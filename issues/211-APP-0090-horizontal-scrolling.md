# 211-APP-0090 — Horizontal scrolling from a trackpad

**Type:** tools (`wm`, `view`, `edit`, `fm`) · **Owner:** tools track (`APP`) · **Priority:** P3 · **Status:** open (planned) · **Blocked by:** — (the across field is in `main` with ABI 15: [211-KRN-0150](../issues-done/211-KRN-0150-an-across-field-in-absolute-pointer-events.done), `pointer_across`) · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Roadmap:** track G · **Constitution:** MC-12.4

**Recorded by:** the kernel track (KRN), 2026-10-09, in `requests-APP.md`, for [211-DRV-0018](211-DRV-0018-macbook-trackpad-gestures.md), at the maintainer's request. Numbered by the tools track on 2026-10-20, at the maintainer's word.

## Problem

`usb_hid` reads the MacBook Pro trackpad's fingers. A three-finger swipe up or down turns the wheel; one left or right turns a horizontal wheel, which nothing reads:

- relative pointer events carry it in bits 34–37 (`pointer_across`);
- an absolute pointer event, the kind a window gets, carries the wheel in those bits and has no field for it (`common/abi.rs`, `pointer_absolute`), so `wm` cannot pass it on.

## Plan

- **The kernel track's part:** an across field in absolute pointer events (`common/abi.rs`, an ABI change), asked in requests-KRN.md.
- **Then the tools track:**
  - `mind::input::Pointer` gains `across`;
  - `wm` passes it to the window under the pointer, as the wheel;
  - `view`, `edit`, `fm` and the text windows scroll sideways by it where their content is wider than the window.

## Acceptance criteria

- A host test of `wm`'s routing passes a horizontal step to the window under the pointer.
- `view` scrolls a wide image sideways by it.
- On the MacBook Pro a three-finger swipe left or right moves a wide image in `view`.

## Related

[211-DRV-0018](211-DRV-0018-macbook-trackpad-gestures.md) (the trackpad's gestures).
