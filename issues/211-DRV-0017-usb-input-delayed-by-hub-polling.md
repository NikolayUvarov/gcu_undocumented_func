# 211-DRV-0017 — USB input a second late on the MacBook Pro: EHCI hub ports polled by control transfers

**Type:** driver · **Owner:** `DRV` (open; made by the kernel session for 211) · **Priority:** P0 · **Status:** in progress (fixed; the MacBook Pro's run left) · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-5.3, MC-11.4

## Problem

With 211-DRV-0016, every keyboard and mouse on the MacBook Pro (MacBookPro10,1, i7-3615QM) worked, the Logitech receiver on xHCI included. But a mouse or the trackpad moved the cursor about once a second (the maintainer, 2026-10-09). LOG:boot0001.log shows reports arriving in bursts: three trackpad reports at 62.020 s, three Logitech reports at 79.030 s.

Two causes in `usb_host` combined.

- **The EHCI scan.** Every 250 ms it asked every port of every EHCI hub for its status, one control transfer each. The Mac has four hubs there:
  - two Intel rate-matching hubs 8087:0024;
  - SMSC 0424:2512;
  - Broadcom 0A5C:4500.
- **Every transfer took at least 10 ms.** `wait_for` spun a thousand times, which is microseconds, then slept in 10 ms ticks. On real hardware a transfer through a transaction translator takes a few frames, so it always reached the sleep.

So `usb_host` spent nearly all its time scanning. `usb_hid`'s requests for reports, one per device every 10 ms, waited behind the scans, and the 8-report queues dropped the rest. QEMU does not show it: no hub sits on its EHCI, and its transfers end at once.

## Plan

- `wait_for` spins for the first 20 ms by the nanosecond clock, then looks every 10 ms. The bounds (`attempts`) stay.
- An EHCI hub's status-change endpoint gets an interrupt pipe on the periodic schedule, as xHCI hubs have. A scan asks only the ports the endpoint names. Every 5 s, or for a hub whose endpoint did not arm or failed, it asks all of them.
- Enable and over-current changes are cleared too, so a port is not named again on every poll.
- An EHCI controller polls up to 16 interrupt endpoints, not 8: four hubs and the HID endpoints exceeded 8.

## Acceptance criteria

- **QEMU:**
  - The `usb` suite passes: a keyboard behind a hub, a tablet, hot plug.
  - A keyboard on EHCI (`usb-ehci` with `usb-kbd,usb_version=2`) types, with a tablet on xHCI, and the reverse.
  - QEMU's hub is full speed and cannot sit on its EHCI, so the hub status endpoint on EHCI is checked only on the Mac.
- **The MacBook Pro:** the cursor follows the trackpad and the Logitech mouse without delay, and keys appear as typed.

## Related

[211-DRV-0016](../issues-done/211-DRV-0016-hid-interfaces-not-ours-claimed-once.done), [211-DRV-0004](211-DRV-0004-ehci.md), `usb_host/src/ehci.rs`, `usb_host/src/xhci.rs`.
