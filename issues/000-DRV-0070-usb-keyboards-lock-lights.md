# 000-DRV-0070 — USB keyboards' lock lights: Caps Lock, Num Lock and Scroll Lock

**Type:** driver · **Owner:** `DRV` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** A · **Constitution:** MC-6.1

From the maintainer's Ryzen PC, 2026-11-13.

## Problem

- **What was seen.** A Lenovo USB keyboard 17EF:60A1 (low speed, a boot keyboard on interface 0, endpoint 81) types in the shell on the Ryzen, but its Caps Lock light never comes on.
- **Why.** `usb_hid` never told a keyboard its locks: it sent no SET_REPORT of the output report. No USB keyboard's lights worked, on any machine.
- **Where the locks live.** `usb_hid` has one decoder (`mind::keys::Ps2`) for all its keyboards.
  - The decoder keeps Caps Lock and Num Lock, and by them chooses the letters' case and what the keypad's keys are.
  - Programs get the characters already in their case.
  - Nothing kept Scroll Lock.
- **A held lock key repeated.** `mind::hid::Keyboard` repeats the last key pressed, and Caps Lock was no exception. Holding it for a second toggled the lock about fifteen times.

## Plan

1. **The lights read the decoder's own state, not a copy.**
   - `Ps2::locks()` gives Num, Caps and Scroll Lock. Scroll Lock is now kept and toggled by its key; Pause (`E1 1D 45 …`) and Ctrl+Break (`E0 46`) do not touch Num Lock or Scroll Lock.
   - So the lights cannot disagree with the case of the letters.
   - With Caps Lock as the layout switch (`keymap --switch caps`), Caps Lock locks nothing and its light stays off.
2. **The boot protocol's output report** (HID 1.11, section 7.2.2 and appendix B.1).
   - `usb_hid` puts every keyboard it serves in the boot protocol. A keyboard in the report protocol only is not served (it is left alone as neither keyboard nor pointer), so there is no report-protocol keyboard to light.
   - In the boot protocol the output report is one byte: Num Lock bit 0, Caps Lock bit 1, Scroll Lock bit 2 (the LED page's usages 1 to 3). Compose and Kana stay off.
   - It is sent with SET_REPORT: bmRequestType 0x21, bRequest 0x09, wValue 0x0200 (output, report ID 0), wIndex the interface. `usb_host` already allows a class request to the claimed interface.
   - The report descriptor is not read for it: in the boot protocol it does not describe the reports.
3. **When.** After each round of polls, every keyboard whose last report sent differs from the locks is sent the new one. That covers a change and a keyboard just set up, so one plugged in after Caps Lock was turned on lights it at once.
4. **A refusal.** It is logged once, with its error, and that keyboard is not sent the report again. Its keys are still read. A device that is gone is let go by its next poll.
5. **Lock keys do not repeat.** A press of Caps, Num or Scroll Lock toggles its lock once and stops the repeat of a key held before it, as on a PS/2 keyboard.
6. **The log**, for each keyboard and each change:
   - `[USB_HID] VVVV:PPPP LOCK LIGHTS: NUM ON|OFF, CAPS ON|OFF, SCROLL ON|OFF (REPORT XX)`;
   - on a refusal: `[USB_HID] VVVV:PPPP LOCK LIGHTS REFUSED (REPORT XX): <error>, NOT SENT AGAIN`.

**Not in this task:**
- the PS/2 keyboard's lights (`ps2_kbd`, command 0xED) and a VirtIO keyboard's (`virtio_input`, its status queue);
- one set of locks for the whole machine: each keyboard driver keeps its own decoder, so a machine with a PS/2 keyboard and a USB keyboard has two;
- a held Caps Lock on a PS/2 keyboard: its typematic repeat sends the make code again, and the shared decoder toggles on each one.

## Acceptance criteria

- **Host tests** (`tests/lock_lights_host.rs`):
  - the report's bits;
  - every keyboard sent all off when it is set up, and then only on a change;
  - Caps Lock on one keyboard lights all of them, and the letters agree;
  - Num Lock and Scroll Lock, and the keypad with Num Lock;
  - a keyboard plugged in after Caps Lock and Num Lock gets them;
  - a held lock key toggles once;
  - Caps Lock as the layout switch lights nothing;
  - Pause and Ctrl+Break toggle no lock.
- **QEMU x86, 4 CPUs:**
  - the `usb` suite: the keyboard is sent `REPORT 00` when it is set up; `caps_lock`, `a`, `num_lock`, `kp_1` type `A` and `1` and send `02`, then `03`; a keyboard plugged in with both on is sent `03` at once, types `M`, and its Caps Lock and Num Lock send `01`, then `00`;
  - the `ehci` suite: the report is taken over EHCI as well as xHCI.
- **QEMU aarch64 `virt`, 4 CPUs:** the same `usb` suite.
- **On the Ryzen, the maintainer** (not done yet):
  - Caps Lock and Num Lock light on 17EF:60A1;
  - a keyboard plugged in after Caps Lock was turned on lights it at once.

## Progress (2026-11-14): on the MacBook Pro, the maintainer

- **Release:** 202611140614 (local tree 8f15853e = `main` 20fba2c5 with this issue's three files and 550-DRV-0069), slot B, over the link. The same build of `usb_hid` ran there first by `replace` (SHA-256 567247c7…).
- **The log:** `05AC:0263 LOCK LIGHTS: NUM OFF, CAPS OFF, SCROLL OFF (REPORT 00)` and the same for `05AC:820A`, at set-up.
- **The maintainer:** Caps Lock lights its key's light, and the letters turn to capitals; pressed again, both go off. A long press acts as a short one (the held key toggles once).
- **Left:** the Ryzen's run as the criteria name it (17EF:60A1, and a keyboard plugged in after Caps Lock). The release with this change also runs on the Ryzen (202611132118) since 2026-11-13; no keyboard was plugged in there for the check yet.

## Related

[213](213-amd-ryzen-pc.md) (the Ryzen), [213-DRV-0061](213-DRV-0061-the-ryzens-usb-keyboard.md) (its other USB keyboard), [164](../issues-done/164-usb-hid-keyboard-and-mouse.done) (`usb_hid`).
