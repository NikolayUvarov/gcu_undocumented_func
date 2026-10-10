# 211-DRV-0021 — A request to a USB device that has gone fails at once

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 · **Status:** in progress (`usb_storage` needs no change; `usb_host`'s part waits, see Plan) · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-6.1, MC-6.6

Numbered from the kernel track's request in `requests-DRV.md` ("A transfer to a device that has gone ends at once", for [211-KRN-0050](211-KRN-0050-a-program-on-an-unplugged-disk-is-refused-at-once.md)).

## Problem

After the maintainer unplugged the MacBook Pro's boot disk, starting a program hung the system:

- `usb_host`'s xHCI `transfer` waits up to about 30 s (`wait`, 4000 attempts) for a completion a gone device never sends, and does not handle the port's change meanwhile;
- `usb_storage` then tried to claim the interface again for 2 s (20 tries, 100 ms apart) on every request.

So each block request to the gone disk cost about 32 s.

## Plan

- **`usb_storage`: no change in the end.** It keeps claiming a lost interface for 2 s (20 tries) on every request:
  - `vfs_server` (211-KRN-0050) stops asking a drive for 10 s after a request that failed after more than 1 s, so the 2 s are spent once in 10 s, not on every request;
  - a device plugged in again or reset is found within them while `usb_host` sets it up.
- **`usb_host`, xHCI and EHCI:**
  - `transfer_for` checks the device's port (PORTSC's connect bit, through the hub for a device behind one) and handles port change events while it waits;
  - a disconnected device's transfer ends at once with an error, and its handle answers `NotFound` from then on;
  - bulk transfers wait about 5 s, not 30.

  This part waits until the kernel session's changes in `usb_host` reach `main` (158's isochronous transfers and the camera, 211-DRV-0016–0018, on `fast-test` now). Changing the same functions from `main` meanwhile would only make conflicts, and `fast-test` is never merged into a branch. The kernel session may also take this part while it works there.

## Acceptance criteria

With the boot disk unplugged (`device_del` in `tests/usb_image_smoke.py`), `usb_host` logs the disconnection, a block read fails within about 2 s (the time `usb_storage` tries the interface), and later ones fail at once for the 10 s `vfs_server` does not ask the drive. Plugged in again, the disk reads as before.

## Progress

**2026-10-09.** `usb_storage`: one claim per request after a loss. With the earlier logging (211-DRV-0019), the MacBook Pro's run on d3da6d0 logged the 2-second loop as `NO ANSWER TO READ` at 56.70 s and `THE DEVICE IS GONE` at 58.70 s. Those two seconds are now gone from every later request.

**2026-10-10.** The one claim per request is withdrawn. In the local gate it broke 211-KRN-0050's test: `vfs_server` takes only a failure that lasted over 1 s for a drive that does not answer, so it never said so, and it asked the gone drive on every request. One claim after the first loss only could fail a request on a device plugged in again that `usb_host` had not set up yet. `usb_storage` claims as on `main`; this task keeps `usb_host`'s part.

## Related

[211-KRN-0050](211-KRN-0050-a-program-on-an-unplugged-disk-is-refused-at-once.md), [211-DRV-0019](211-DRV-0019-usb-storage-refusals-and-resets.md), `usb_storage/src/main.rs`, `usb_host/src/xhci.rs`, `usb_host/src/ehci.rs`.
