# 000-KRN-0149 — A key code for PrintScreen

**Type:** ABI (`KEY_PRINT_SCREEN`, ABI 15), `libmind` key decoder · **Owner:** kernel session · **Priority:** P3 · **Status:** made (2026-10-22); the gate left · **Blocked by:** — · **Main task:** — (for [000-APP-0089](000-APP-0089-screenshots-and-recordings-for-the-page.md)) · **Constitution:** MC-12.4

From the tools track's request in [requests-KRN.md](requests-KRN.md) (2026-10-20), numbered here and taken into ABI 15 with [800-KRN-0148](800-KRN-0148-a-shell-client-for-link-in-developer-mode.md).

## Problem

- **The maintainer asked for a screenshot key,** PrintScreen on a PC.
- **No key event names it.**
  - The scan code decoder in `libmind/src/keys.rs` drops `E0 37`.
  - `common/abi.rs` has no `KEY_*` code for it, so the shell cannot `listen` for it.
  - `usb_hid` already turns HID usage `0x46` into `E0 37`.

## Plan

- **`KEY_PRINT_SCREEN` = 33** in `common/abi.rs`, the first free code after `KEY_POINTER`. A new key code is an ABI change; it rides ABI 15.
- **The decoder** gives it for `E0 37`. The fake shifts around it (`E0 2A`, `E0 AA`) stay silent, as they are now.

## Acceptance criteria

- [x] A host test of the decoder gives `KEY_PRINT_SCREEN` for `E0 2A E0 37` and nothing for its release `E0 B7 E0 AA`.
- [ ] The gate passes. The tools track then listens for it beside Ctrl+Alt+F6.

## Progress (2026-10-22)

- `KEY_PRINT_SCREEN` = 33 in `common/abi.rs`; the decoder gives it for `E0 37` (`libmind/src/keys.rs`).
- `tests/keys_host.rs` `ps2_print_screen_through_its_fake_shift`: `E0 2A E0 37 E0 B7 E0 AA` gives one PrintScreen and leaves no shift held.
- `INPUT_LISTEN` takes any key code but 0 and `KEY_POINTER`, so the shell may listen for it as is.

## Related

[000-APP-0089](000-APP-0089-screenshots-and-recordings-for-the-page.md), [800-KRN-0148](800-KRN-0148-a-shell-client-for-link-in-developer-mode.md) (ABI 15).
