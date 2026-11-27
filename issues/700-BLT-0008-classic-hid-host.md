# 700-BLT-0008 — The classic HID host: a Bluetooth keyboard and mouse into the system's input as `usb_hid` delivers

**Type:** library (`bt_host`) and driver (`bt_hci`) · **Owner:** `BLT` · **Priority:** P1 · **Status:** in progress (2026-11-26) · **Blocked by:** `KRN`: the input privilege for `bt_hci` (asked on `internal-dev-messages`, 2026-11-26) · **Main task:** [700](700-bluetooth.md) · **Constitution:** MC-3.5, MC-11.4, MC-12.1, MC-12.3

## Problem

A Bluetooth keyboard or mouse is a HID device on L2CAP. Its input has to reach the focused program as a USB one's does.

## Plan

1. **`hid.rs`: the control channel (PSM 0x0011), then the interrupt channel (0x0013).**
   - We open them on a link we made, after it is secure. A device that reconnects opens them itself, and we admit them.
   - On every connection, Set Protocol puts the device in the boot protocol, which every keyboard and mouse has. The reports then have the layout `usb_hid` reads.
2. **Input reports.**
   - A keyboard's (report ID 1, 8 bytes) goes to `mind::hid::Keyboard` and the PS/2 decoder, with key repeat, then `mind::keyboard::deliver`. This is `usb_hid`'s path.
   - A mouse's (ID 2: buttons, X, Y, wheel) goes to `mind::hid::Pointer::boot()` and the input event.
   - Held keys are released when the link goes.
3. **The keyboard's lights:** an output report on the interrupt channel.
4. **Virtual cable unplug:** logged, and the channels closed.
5. **The input privilege** is `init`'s to grant `bt_hci`, as it grants `usb_hid` (`SLOT_PRIV`, `CAP_KIND_INPUT`). Asked of KRN. Without it the reports arrive and are dropped, and the start line says `HID INPUT NOT DELIVERED`.

**What is not done.**
- The report protocol with the SDP HID descriptor (a device that refuses the boot protocol: its reports are taken as they come).
- The lights from the keyboard service's locks (the call exists in `bt_host`; `bt_hci` does not serve `keyboard.wit` yet).
- A HID device lent to one program (issue 700's plan).

## Acceptance criteria

- [x] Host tests (`bt_host/tests/sdp_hid.rs`):
  - a keyboard: the channels, the boot protocol, a key pressed and released, other reports left out, the lights, keys released when the link goes;
  - a mouse that reconnects and opens its own channels, and its report.
- [ ] With KRN's grant, on the Ryzen: the maintainer's keyboard types in the shell, and a mouse moves the cursor.

## Progress (2026-11-26)

- **The report path is shown through LE, not through a classic device.** The maintainer's mouse is LE, so this task's classic HID host was not tried. With the bench branch's input privilege, its reports went through `bt_hci`'s path to the input (700-BLT-0009): `HID: n REPORTS …; INPUT CALLS … OK, 0 FAILED`, and the pointer and both buttons moved for the maintainer.
- **For `main`:** the first report delivered is logged, then a tally once a minute (e4f5c307f).
- **Not tried:** a classic Bluetooth keyboard or mouse, and the maintainer's Apple keyboard.

## Related

[700-BLT-0005](700-BLT-0005-connections-carry-data.md), [700-BLT-0006](../issues-done/700-BLT-0006-classic-pairing-and-link-keys.done), [700-BLT-0009](../issues-done/700-BLT-0009-le-pairing-and-hid-over-gatt.done) (HID over GATT).
