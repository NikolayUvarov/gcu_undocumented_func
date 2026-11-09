# 214-DRV-0062 — The boot stick not set up on a USB 2 port of an Intel xHCI

**Type:** driver · **Owner:** `DRV` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [214](214-intel-core-i5-pc.md) · **Constitution:** MC-6.1, MC-12.1

Numbered from the kernel track's request of 2026-10-13 in `requests-DRV.md`, from a photo of the screen of the maintainer's Intel Core i5-4430 PC and its later logs.

## Problem

**What happened:**
- The bootloader started from the stick, the services started, and the keyboard typed.
- Then `usb_host` logged `PORT 8: DEVICE NOT SET UP AT ADDRESS DEVICE (COMPLETION 1, PORTSC 00000E03, TRY 3 OF 3)`, then `NO MASS STORAGE DEVICE`.
- `vfs_server` found no boot volume, so no program could start and no log was written.

**What is known:**
- **The controller:** the H81's xHCI (8086:8C31) has ports 1–10 for USB 2 and 16–17 for USB 3, and every port was routed to it.
- **The stick** is a SuperSpeed one (7825:A2A4, the model the MacBook Pro boots from). It came up at high speed on port 8.
- **Not every time:** on boot 1 the same stick was set up on the same port, with the same build.

**The line names the wrong command's code:**
- `enumerate` gives up at Address Device;
- `discard` then disables the slot, and Disable Slot's success (1) overwrites `xhci.last`.

## Plan

1. **Keep the failing command's code** for the line: save it before `discard`, or log it in `enumerate`.
2. **A high-speed device on an xHCI USB 2 port:**
   - the 10 ms reset recovery (USB 2.0 TRSTRCY) before Address Device on a root port as on a hub's;
   - EP0's packet of 64;
   - where the device needs the time, an Address Device with BSR=1 first, then one without.
3. **The maintainer's run:** several boots with the stick in a black port, from power-off and by `reboot`, until the line shows Address Device's own code, or until the stick comes up every time.

## Acceptance criteria

- On that PC the stick is set up on its USB 2 ports in each of the runs' boots.
- When Address Device fails, the line names its own completion code.

The runs need the maintainer at the PC.

## Related

[214](214-intel-core-i5-pc.md), [214-DRV-0063](214-DRV-0063-the-intel-i5-pcs-sound-and-ethernet.md), [211-DRV-0053](211-DRV-0053-a-usb-disk-that-missed-a-transfer-comes-back.md).
