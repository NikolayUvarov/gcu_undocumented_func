# 164 — USB keyboards and mice: a USB host driver and a HID class driver (real machines without PS/2)

**Type:** drivers · **Owner:** kernel track (requested by the porting track) · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Roadmap:** track A (drivers), needed by track H (boards, 205) · **Constitution:** MC-1.1, MC-3.3, MC-6.1, MC-6.3, Appendix B.4

## Problem

MIND Core takes keys only from a PS/2 controller (`ps2_kbd`, `LEGACY:`) or a VirtIO keyboard (`virtio_input`, QEMU). Most real machines have neither: Intel Macs have no PS/2 controller and no emulation of one, laptops' and desktops' keyboards and mice are USB, and ARM boards (Raspberry Pi 4, issue 205) have only USB. On such a machine the system boots to the shell and cannot be used.

The only USB code is in `usb_storage` (`usb_storage/src/xhci.rs`): it resets the xHCI controller, addresses devices until it finds one mass-storage interface, and skips the rest. Issue [158](158-video-capture.md) plans a general USB stack as its first step; keyboards need it first, so that step is done here and 158 builds on it.

## Plan

1. **`usb_host`** (ring 3, the xHCI controller's BAR, its MSI-X vector or IRQ, a DMA region): takes the controller from firmware (USB legacy handoff), ports of USB 2 and 3, enumeration with addresses, device and configuration descriptors, **hubs** (internal keyboards often sit behind one: Intel Macs, many laptops), control, bulk and interrupt transfers, events through interrupts with polling as a fallback. One interface of one device is handed to one class driver; a class driver sees only its interface (MC-3.3).
   - Interface `idl/usb.wit` 1.0 between the host driver and class drivers: claim an interface, its endpoints, transfers, detach notices.
   - `usb_storage` becomes the first class driver over it (behaviour unchanged).
2. **`usb_hid`** (class driver): HID keyboards and mice, boot protocol first (8-byte keyboard reports, 3–4-byte mouse reports), several devices at once, hot plug.
   - Keys: HID usages are translated to the PS/2 set-1 codes `mind::keys::Ps2` decodes (as `virtio_input` does for VirtIO keyboards); `usb_hid` serves the keyboard service (`idl/keyboard.wit`) and the keyboard LEDs.
   - Pointer: relative movement, buttons and wheel through the same input events as `virtio_input`'s mouse; an absolute device (QEMU's `usb-tablet`) as absolute pointer events (issue 161).
   - `init` lends `SLOT_KEYBOARD` from `usb_hid` when there is no `ps2_kbd` (as it does for `virtio_input` now).
3. **Supervision:** a fault in `usb_hid` does not stop `usb_host` or `usb_storage`; a restarted `usb_host` resets the controller and the class drivers attach again (B.4: stop DMA before memory is reused).
4. **Both architectures:** the same services on x86 and aarch64 (`virt` with `qemu-xhci`); this is the USB keyboard of issue 205.
5. **Not in this issue:** EHCI-only machines (USB 2 controllers before about 2012, older Macs) — a separate issue if wanted; Apple's SPI keyboards and trackpads (MacBooks from 2015); multitouch trackpad gestures (the trackpad in its mouse mode is enough).

## Acceptance criteria

- **QEMU x86 and aarch64** (CI): with `-device qemu-xhci -device usb-kbd -device usb-mouse -device usb-tablet` and no PS/2 or VirtIO keyboard in use, keys sent to the USB keyboard (QMP `input-send-event` with its device id) reach the shell, including Shift, Ctrl, arrows and Cyrillic layout switching; the mouse moves the pointer in `wm` and the tablet places it absolutely; a keyboard plugged and unplugged at run time (QMP `device_add` / `device_del`) appears and goes.
- **USB storage** unchanged on `usb_host`: the USB image suite and the `ahci`/USB suites pass.
- **Faults:** killing `usb_hid` and `usb_host` in turn; each is restarted and the keyboard works again.
- **Host tests:** HID report decoding (keyboard rollover, modifiers, mouse deltas), descriptor parsing with malformed input.
- **Profiles:** `docs/profile` (x86 and aarch64) list the new drivers with their DMA and their evidence; a manual run on a real machine with a USB keyboard (a PC, an Intel Mac from 2012 on) is recorded with the model when one is done.

## Related

[158](158-video-capture.md) (its step 1 is this issue's step 1), [205](205-aarch64-boards.md), [161](../issues-done/161-absolute-pointer-virtio-tablet.done), [docs/legacy.md](../docs/legacy.md).
