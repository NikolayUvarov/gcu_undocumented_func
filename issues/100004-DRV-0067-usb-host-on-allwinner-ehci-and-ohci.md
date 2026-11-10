# 100004-DRV-0067 — `usb_host` on the Allwinner EHCI and OHCI in memory

**Type:** driver · **Owner:** `DRV` · **Priority:** P3 · **Status:** open (later: after a board boots) · **Blocked by:** [100004-MOB-0002](100004-MOB-0002-the-first-boot-on-the-h5.md) or [0003](100004-MOB-0003-the-first-boot-on-the-h618.md) · **Main task:** [100004](100004-two-allwinner-boards-before-the-phone.md); [100001](100001-telephony-sms-and-calls-through-the-modem.md) (the PinePhone's modem is on USB) · **Constitution:** MC-1.5, MC-2.3, MC-12.1

## Problem

**What `usb_host` drives today:** xHCI controllers on PCI.

**What the Allwinner SoCs have** (the A64, the H5, the H618):
- USB 2.0 EHCI controllers, each with an OHCI companion for full- and low-speed devices, in memory and named by the device tree (`generic-ehci`, `generic-ohci`, with their clocks, resets and PHY);
- a MUSB OTG port.

**Where it is needed:**
- **The boards:** a USB keyboard, a stick, a USB-UART adapter or the Quectel modem the handoff names for the Orange Pi Zero 3.
- **The PinePhone:** the EG25-G modem sits on the A64's EHCI.

## Plan

1. **The controllers by their nodes** ([100000-KRN-0171](../issues-done/100000-KRN-0171-platform-devices-from-the-device-tree.done)): EHCI first, for high-speed devices and hubs; OHCI for the full-speed devices on a root port (the CH340, keyboards). Their PHY and clocks as U-Boot left them, until the clock controller's service exists.
2. **The same `usb.wit` for the class drivers,** so that `usb_hid`, `usb_storage` and `usb_serial` do not change.
3. **Host tests** of the schedules (EHCI's queue heads, OHCI's endpoint descriptors) against a model. QEMU's `virt` takes `-device usb-ehci` on PCI, so the controller's logic is tested there too, before a board.

## Acceptance criteria

- **On a board:** a USB keyboard types at the shell, and a stick is read.
- **The configuration** is recorded: the board, the device, the port (MC-12.1).

## Related

[100004](100004-two-allwinner-boards-before-the-phone.md), [100001](100001-telephony-sms-and-calls-through-the-modem.md), [164](../issues-done/164-usb-hid-keyboard-and-mouse.done).
