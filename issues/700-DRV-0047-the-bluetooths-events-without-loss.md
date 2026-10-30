# 700-DRV-0047 — `usb_host` keeps a Bluetooth controller's events without loss

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 · **Status:** in progress (2026-10-31) · **Blocked by:** — · **Main task:** [700](700-bluetooth.md) · **Constitution:** MC-12.1

## Problem

- **What the DEXP showed.** Once `rtw88_wifi` let the RTL8821C's Bluetooth on air (700-DRV-0046), `bt_hci`'s rounds logged packets dropped and no inquiry complete (700-BLT-0002):
  - `64 EVENT PACKETS DROPPED` in one round, `195` in another;
  - `NO INQUIRY COMPLETE IN 12000 MS`.
- **Why.** `usb_host`'s xHCI side keeps 8 reports of an interrupt IN endpoint, and drops the oldest when a ninth comes (`usb_host/src/xhci.rs`).
  - The controller sends a 16-byte event packet a millisecond. `bt_hci` takes them about every 10 ms.
  - An extended inquiry result is 17 packets, and a lost one breaks the event it belongs to.
  - A mouse loses a stale movement that way; an HCI event is lost whole.
- **The remedy exists.** `usb_host` already holds a converter's bulk IN endpoint without loss (800-DRV-0034): a transfer goes back on the ring only while the queue has room, so the device waits (NAK) instead of a report being dropped.

## Plan

1. **In `reports`:** the endpoint is armed `lossless` when it is bulk, or when the client asking is badged `BADGE_BLUETOOTH` (`bt_hci`, 700-DRV-0043). Nothing else changes for other clients.
2. **EHCI's reports** (`ehci_reports`) are not changed here. The DEXP's controller is on xHCI.

## Acceptance criteria

- [x] The x86 and aarch64 builds and the QEMU `usb` suite pass. QEMU has no Bluetooth controller, so this only shows that nothing else changed.
- [ ] On the DEXP, with 700-DRV-0046 and 700-BLT-0002: `bt_hci`'s rounds drop no event packet, and the inquiry completes. Evidence for the DEXP only (MC-12.1).

## Progress (2026-10-31)

- **Built** for x86 and aarch64 without warnings. The QEMU `usb` suite passed on x86 (LEG9, WSL2).
- **Before it, on the DEXP:** 700-BLT-0002's rounds dropped 64, 195 and 404 packets, and no inquiry completed.
- **Still to see:** the DEXP with it. `usb_host` cannot be replaced by `effector485 replace`, as the boot volume is a USB stick on it, so this waits for a slot update.

## Related

[700-DRV-0046](700-DRV-0046-rtw88-gives-the-bluetooth-its-antenna.md), [700-DRV-0043](700-DRV-0043-usb-host-hands-out-bluetooth-controllers.md), [800-DRV-0034](800-DRV-0034-usb-host-serves-serial-converters.md); BLT's 700-BLT-0002.
