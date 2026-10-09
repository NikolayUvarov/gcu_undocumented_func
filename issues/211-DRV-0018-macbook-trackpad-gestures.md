# 211-DRV-0018 — The MacBook Pro's trackpad: fingers, a right click with two, scrolling with three

**Type:** driver · **Owner:** `DRV` (open; made by the kernel session for 211, at the maintainer's request) · **Priority:** P1 · **Status:** in progress (made and host-tested; the MacBook Pro's run left) · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-11.4, MC-12.4

## Problem

With 211-DRV-0016 and 0017 the trackpad of the MacBook Pro (05AC:0263, Wellspring 7) moves the cursor, but only as a mouse: interface 2 in its mouse mode gives one button and a movement, nothing of how many fingers touch. On 2026-10-09 the maintainer asked for three gestures:
- a click with two fingers is the right button;
- three fingers moved up or down scroll vertically;
- three fingers moved left or right scroll horizontally.

The trackpad tells its fingers only in its multitouch mode, on interface 1, which `usb_hid` left alone. Linux's bcm5974 driver documents the protocol; only its facts are taken, not its code (GPL):
- **The mode switch.** Read feature report 0 of interface 0 (GET_REPORT, value 0x300, 8 bytes). Set byte 0 to 1, and write it back (SET_REPORT).
- **The packets.** They come from interface 1's endpoint 81. Each is a 30-byte header with the built-in button at byte 15, then 28 bytes a finger:
  - X at 2 and Y at 4, signed, Y growing upwards;
  - the touch's major axis at 16, zero for a lifted finger.

  With up to 16 fingers a packet takes 478 bytes, several 64-byte USB packets ended by a short one. `usb.wit` could not carry that: one report a transfer of one packet, a length byte each.

## Plan

- **`idl/usb.wit` 1.1** adds `reports-up-to(handle, address, longest)`: reports of several packets of up to `longest` bytes (at most 512), each after two length bytes.
  - Adding a function at the end raises the minor version (docs/idl). Old clients and servers are unaffected.
  - xHCI keeps one packet of up to 64 bytes for now; the Mac's trackpad is on EHCI.
- **`usb_host`, EHCI.** Two endpoints may poll with 512-byte descriptors, in a DMA area of their own; the others keep 64 bytes.
- **`libmind::hid`:**
  - `wellspring(vendor, product)` names the trackpads with this layout: Wellspring 5 to 7A, MacBook Pro 8 to 10 and MacBook Air 4 to 5.
  - `WELLSPRING_MODE` is the switch.
  - `Trackpad::feed` turns packets into pointer events:
    - one finger moves the pointer, faster strokes further;
    - pressing the pad is the left button, or the right one with two fingers on it;
    - a quick two-finger tap (under 250 ms, little travel) is a right click;
    - three fingers scroll, vertically or horizontally by the way they first move, in the content's direction (macOS's natural scrolling).
- **`common/abi.rs`:** a relative pointer event's horizontal wheel in bits 34–37 (`pointer_scroll`, `pointer_across`). Those bits are zero in every other relative event; absolute events keep their wheel there.
- **`usb_hid`:**
  - For such a device, interface 1 is switched through the keyboard's interface 0 and read with `reports-up-to`. Interface 2, the mouse, is then left alone.
  - If the switch is refused, the mouse interface serves as before.
  - It holds up to 8 interfaces, not 4.
- **Horizontal scrolling** is delivered; `wm` and the programs reading it is requested from `APP` ([requests-APP.md](requests-APP.md)).

## Acceptance criteria

- **Host tests (`tests/hid_host.rs`):**
  - motion and its speed;
  - the left and right button, and a right button held while a finger lifts;
  - the two-finger tap, and what is not one;
  - three-finger scrolling on one axis at a time, in both directions;
  - short packets.
- **QEMU:** the `usb` suite passes. QEMU has no Apple trackpad, so the gestures are checked only on the Mac.
- **The MacBook Pro:**
  - the log says `TRACKPAD: MULTITOUCH MODE`;
  - one finger moves the cursor smoothly;
  - a press is a left click, a press with two fingers or a two-finger tap a right click;
  - three fingers up and down scroll text in `wm`'s windows.

## Progress

**2026-10-09: the first build stopped every USB device on the MacBook Pro.** No keyboard, mouse or USB drive worked, and the log volume got no `acpi/` folder: the log held two lines, from 1969 s.

- **The cause.** Each interrupt endpoint's report queue was made for 512-byte reports. That grew the EHCI controller's state to about 66 KiB, and it is built on `usb_host`'s stack, which is 64 KiB. `usb_host` overflowed it when it set up an EHCI controller.
- **Why the tests missed it.** QEMU's suites have no EHCI controller, and the EHCI check (a keyboard on `usb-ehci`) was not run again before the push.
- **The fix.** A queue is allocated on the heap at the size its endpoint polls with: 64 bytes a report, 512 only for the two long endpoints. `usb_host` now uses libmind's heap. The EHCI check runs before every push of `usb_host`.

## Related

[211-DRV-0017](211-DRV-0017-usb-input-delayed-by-hub-polling.md), [211-DRV-0016](../issues-done/211-DRV-0016-hid-interfaces-not-ours-claimed-once.done), `libmind/src/hid.rs`, `usb_hid/src/main.rs`, `usb_host/src/ehci.rs`.
