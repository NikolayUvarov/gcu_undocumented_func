# 211-DRV-0008 — A USB mouse on real hardware: the report protocol, and interfaces whose setup fails

**Type:** driver · **Owner:** `DRV` (open) · **Priority:** P2 · **Status:** in progress · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-12.1

## Problem

On the MacBook Pro of [211-PRT-0004](211-PRT-0004-first-run-on-an-intel-pc.md), the keyboard of a Logitech receiver (046D:C534) types, but its mouse moves no pointer, and `keys` shows no pointer events. `usb_host` logs the mouse endpoint's first report as 4 bytes. That is the size of a boot-protocol report: buttons, X, Y, the wheel. Its report descriptor gives the mouse report ID 2, so `usb_hid` takes the first byte for the ID and drops every report. The firmware is the likely cause: it drives a USB mouse in the boot protocol, and `usb_hid` never asked for the report protocol.

**A mouse that moved only sideways** (the maintainer, 2026-10-12, `fast-test` c548a85): "I plugged in another mouse. It moves only left and right, while Windows reads its movements properly." The boot log shows:

- `10C4:8105 MOUSE: ID 0, X AT BIT 8 (8 BITS), Y AT BIT 16 (8 BITS), BOOT PROTOCOL`: `usb_hid` found no X and Y in its report descriptor and fell back to the boot protocol;
- its reports are 6 bytes, `[00, 03, 00, FC, FF, 00]`: buttons, then X and Y of 16 bits each (+3 and −4), then the wheel. The mouse kept its own layout although it was asked for the boot protocol;
- read with the boot layout, X is its low byte, Y is X's high byte (0 or −1), and Y's low byte turns the wheel. So the pointer moved only sideways.

Why the descriptor gave no X and Y, as read from the code:

- **`usb_host`'s xHCI control transfers** returned the length asked, not the length that came: the data stage did not ask for an event on a short packet (ISP), so its residue was never seen. `usb_hid` asks for up to 1024 bytes of report descriptor, so it got the descriptor followed by whatever earlier transfers had left in the buffer. In QEMU that was zeros, harmless; on a real machine it may be anything. The EHCI path (the MacBook Pro's own keyboard and trackpad) counted the bytes right.
- **`mind::hid`'s parser** gave up on the whole descriptor at an item cut short, which bytes left over can end with.
- **It took each usage's page when the usage came.** Some mice give X and Y, then the Generic Desktop page, so X and Y were read on the button page. Linux's `hid-core` gives such usages the page in force at the main item.

Which of these struck this mouse is not known: its descriptor was not logged.

**The second run** (the maintainer, 2026-10-12, `fast-test` 3ab50c0, with the changes below): the mouse still moved only sideways, and the log said why: `10C4:8105 REPORT DESCRIPTOR: 0 BYTES`. Asked for up to 1024 bytes, the mouse sent none. Its firmware probably reads only wLength's low byte, and 1024 is 0x400. Windows and Linux ask for the length that the HID descriptor in the configuration descriptor names, so the mouse works there. Before the first change `usb_host` reported the 1024 bytes asked, so the empty answer looked like a descriptor followed by stale bytes.

A second device there, 1A81:1006 (a low-speed keyboard and mouse), fails its requests after Configure Endpoint. `usb_hid` gave the failing interface back and claimed it again at once, so its log filled with that interface, many times a second.

## Plan

- **The report protocol.** A boot-subclass interface laid out by its report descriptor is sent `SET_PROTOCOL` (report), as HID 1.11 section 7.2.6 asks of the host.
- **A boot mouse that keeps the boot protocol.** If a boot mouse whose descriptor declares report IDs sends reports that start with none of them, it is switched to the boot protocol and its boot layout.
- **What the driver saw.** `usb_hid` logs:
  - the layout it takes: the report ID, and the bit offset and size of X and Y;
  - the protocol;
  - a pointer's first three reports.

  `usb_host` logs the first bytes of an endpoint's first report.
- **Report descriptors read whole and only once** (2026-10-12):
  - `usb_host`'s xHCI control transfers ask for an event on a short IN data stage (ISP) and return the bytes that came, after waiting for the status stage's completion as well;
  - `mind::hid` keeps what came before an item cut short;
  - usages given without a page take the page in force at the main item, back to the first already on it, as Linux's `hid-core` does;
  - `usb_hid` logs a HID interface's report descriptor (up to 160 bytes) when it is first set up, and why it fell back to the boot protocol: no X and Y in the descriptor, or the descriptor not read.
  - `usb_hid` asks for the report descriptor at the length its HID descriptor names. It finds that length in the configuration descriptor, read at its own length: 9 bytes, then wTotalLength. Without one it asks for 1024 bytes, then 255 if nothing came.
- **A failed interface.** It stays claimed and is tried again every 5 s, at most 5 times, while the other interfaces are claimed and served. A device that is gone is given back at once.
- **Test.**
  - The QEMU `usb` suite plugs in a boot mouse at run time and checks the layout, the logged report and the movement in `keys`.
  - It also checks that the descriptors logged are shorter than the 1024 bytes asked. Before the change, QEMU's mouse and tablet logged 1024; after it, 52 and 74.
  - `tests/hid_host.rs` reads the descriptor of a mouse with 16-bit X and Y, with the page before and after its usages, and its report `[00, 03, 00, FC, FF, 00]` as +3 and −4. The test checks that a descriptor cut short keeps its layout. It also finds each interface's report descriptor length in configuration descriptors: QEMU's mouse, and a device with two interfaces.

## Acceptance criteria

- The Logitech receiver's mouse moves the pointer on the MacBook Pro.
- The mouse 10C4:8105 moves the pointer both ways on the MacBook Pro. Its descriptor is in the boot log.
- A device whose interface fails its setup does not flood the log, and the other devices work.
- The QEMU `usb` suite passes with the mouse on x86 and aarch64.

## Progress

- 2026-10-08, the MacBook Pro, `fast-test` at 89809ad: the Logitech receiver's mouse works. The cursor moves, both buttons work, the desktop menu opens on a right click and windows are dragged. The QEMU `usb` suite passes with the boot mouse on x86. Still to see: the log of the low-speed device 1A81:1006, now that its interface is retried every 5 s instead of claimed again at once; the suite on aarch64.

- 2026-10-12, the MacBook Pro, `fast-test` 7281472 (the length from the configuration's HID descriptor, `log:boot0001.log`): `10C4:8105 REPORT DESCRIPTOR: 64 BYTES`, dumped whole. `usb_hid` reads it as `MOUSE: ID 0, X AT BIT 8 (16 BITS), Y AT BIT 24 (16 BITS), REPORT PROTOCOL`, and a report `[00, 00, 00, FF, FF, 00]` is a move of −1 in Y. **Left:** the maintainer's word that the pointer moves up and down.

## Related

[211-DRV-0003](../issues-done/211-DRV-0003-usb-host-on-real-hardware.done), [211-DRV-0004](211-DRV-0004-ehci.md), issue 164.
