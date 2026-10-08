# 211-DRV-0008 — A USB mouse on real hardware: the report protocol, and interfaces whose setup fails

**Type:** driver · **Owner:** `DRV` (open) · **Priority:** P2 · **Status:** in progress · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-12.1

## Problem

On the MacBook Pro of [211-PRT-0004](211-PRT-0004-first-run-on-an-intel-pc.md), the keyboard of a Logitech receiver (046D:C534) types, but its mouse moves no pointer, and `keys` shows no pointer events. `usb_host` logs the mouse endpoint's first report as 4 bytes. That is the size of a boot-protocol report: buttons, X, Y, the wheel. Its report descriptor gives the mouse report ID 2, so `usb_hid` takes the first byte for the ID and drops every report. The firmware is the likely cause: it drives a USB mouse in the boot protocol, and `usb_hid` never asked for the report protocol.

A second device there, 1A81:1006 (a low-speed keyboard and mouse), fails its requests after Configure Endpoint. `usb_hid` gave the failing interface back and claimed it again at once, so its log filled with that interface, many times a second.

## Plan

- **The report protocol.** A boot-subclass interface laid out by its report descriptor is sent `SET_PROTOCOL` (report), as HID 1.11 section 7.2.6 asks of the host.
- **A boot mouse that keeps the boot protocol.** If a boot mouse whose descriptor declares report IDs sends reports that start with none of them, it is switched to the boot protocol and its boot layout.
- **What the driver saw.** `usb_hid` logs:
  - the layout it takes: the report ID, and the bit offset and size of X and Y;
  - the protocol;
  - a pointer's first three reports.

  `usb_host` logs the first bytes of an endpoint's first report.
- **A failed interface.** It stays claimed and is tried again every 5 s, at most 5 times, while the other interfaces are claimed and served. A device that is gone is given back at once.
- **Test.** The QEMU `usb` suite plugs in a boot mouse at run time and checks the layout, the logged report and the movement in `keys`.

## Acceptance criteria

- The Logitech receiver's mouse moves the pointer on the MacBook Pro.
- A device whose interface fails its setup does not flood the log, and the other devices work.
- The QEMU `usb` suite passes with the mouse on x86 and aarch64.

## Related

[211-DRV-0003](../issues-done/211-DRV-0003-usb-host-on-real-hardware.done), [211-DRV-0004](211-DRV-0004-ehci.md), issue 164.
