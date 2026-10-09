# 211-DRV-0021 — A request to a USB device that has gone fails at once

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 · **Status:** in progress (`usb_storage`'s part done; `usb_host`'s part waits, see Plan) · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-6.1, MC-6.6

Numbered from the kernel track's request in `requests-DRV.md` ("A transfer to a device that has gone ends at once", for [211-KRN-0050](211-KRN-0050-a-program-on-an-unplugged-disk-is-refused-at-once.md)).

## Problem

After the maintainer unplugged the MacBook Pro's boot disk, starting a program hung the system:

- `usb_host`'s xHCI `transfer` waits up to about 30 s (`wait`, 4000 attempts) for a completion a gone device never sends, and does not handle the port's change meanwhile;
- `usb_storage` then tried to claim the interface again for 2 s (20 tries, 100 ms apart) on every request.

So each block request to the gone disk cost about 32 s.

## Plan

- **`usb_storage`, done:**
  - claims a lost interface once per request, without the loop, so a request fails at once while nothing is there;
  - finds the disk again on the first request after it is plugged in;
  - keeps the 20 tries at start only, where `usb_host` may still be setting the device up.
- **`usb_host`, xHCI and EHCI:**
  - `transfer_for` checks the device's port (PORTSC's connect bit, through the hub for a device behind one) and handles port change events while it waits;
  - a disconnected device's transfer ends at once with an error, and its handle answers `NotFound` from then on;
  - bulk transfers wait about 5 s, not 30.

  This part waits until the kernel session's changes in `usb_host` reach `main` (158's isochronous transfers and the camera, 211-DRV-0016–0018, on `fast-test` now). Changing the same functions from `main` meanwhile would only make conflicts, and `fast-test` is never merged into a branch. The kernel session may also take this part while it works there.

## Acceptance criteria

With the boot disk unplugged (`device_del` in `tests/usb_image_smoke.py`), a block read fails within 1 s, and `usb_host` logs the disconnection. Plugged in again, the disk reads as before.

## Progress

**2026-10-09.** `usb_storage`: one claim per request after a loss. With the earlier logging (211-DRV-0019), the MacBook Pro's run on d3da6d0 logged the 2-second loop as `NO ANSWER TO READ` at 56.70 s and `THE DEVICE IS GONE` at 58.70 s. Those two seconds are now gone from every later request.

## Related

[211-KRN-0050](211-KRN-0050-a-program-on-an-unplugged-disk-is-refused-at-once.md), [211-DRV-0019](211-DRV-0019-usb-storage-refusals-and-resets.md), `usb_storage/src/main.rs`, `usb_host/src/xhci.rs`, `usb_host/src/ehci.rs`.
