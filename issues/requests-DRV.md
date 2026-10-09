# Requests for the drivers track (DRV), not numbered yet

**Owner:** drivers track (the assessing session, `claude/ASR-DRV`, TRACKS 1.5) · **Status:** open · **Recorded by:** the kernel track (KRN), 2026-10-09

The drivers track numbers its own tasks (`NNN-DRV-MMMM`), so requests from other tracks wait here. The drivers track turns each into a task and removes it from this file, and the file goes when it is empty.

## The MacBook Pro's speakers: more of 551's step 2 if the GPIO is not enough (551)

**Recorded by:** the kernel track (KRN), 2026-10-09, for [551-DRV-0010](551-DRV-0010-hda-controller-and-codecs.md). That task stays with the kernel session until done (TRACKS 1.5).

The kernel session has set the Cirrus codec's amplifier GPIOs for Apple's machines (551-DRV-0010's progress) because the maintainer's run found `beep` and `say` silent. Jack detection, the internal microphone and any further codec setup are 551's step 2, which this track numbers when it takes it.

## A transfer to a device that has gone ends at once (211-KRN-0050)

**Recorded by:** the kernel track (KRN), 2026-10-09, for [211-KRN-0050](211-KRN-0050-a-program-on-an-unplugged-disk-is-refused-at-once.md), after the maintainer unplugged the MacBook Pro's boot disk: starting a program then hung the system.

### Problem

- **`usb_host`, xHCI.**
  - `transfer` waits up to about 30 s (`wait`, 4000 attempts) for a completion. A device that was unplugged never sends one.
  - While it waits, the port's change is not handled: the Mac's log has no `PORT 5: DISCONNECTED`, only the `CONNECTED` of the replug.
  - So every bulk transfer to the gone disk takes 30 s.
- **`usb_storage`.** After a failed command `run` tries to claim the interface again for 2 s (20 tries, 100 ms apart) on every request.
- **The cost.** About 32 s per block request. `vfs_server` now stops asking a drive that does not answer for 10 s, but each new try still costs the 32 s.

### Plan (a proposal; the drivers track decides)

- `transfer_for` checks the device's port (PORTSC's connect bit, through the hub for a device behind one) and handles port change events while it waits. A disconnected device's transfer ends at once with an error, and its handle answers `NotFound` from then on.
- Bulk transfers wait about 5 s, not 30.
- `usb_storage` claims a gone interface once per request, without the 2-second loop. It answers at once while nothing is there, and finds the disk again when it is plugged in.
- The same on EHCI (`ehci.rs`, `wait_for`).

### Acceptance criteria

With the boot disk unplugged (`device_del` in `tests/usb_image_smoke.py`), a block read fails within 1 s, and `usb_host` logs the disconnection. Plugged in again, the disk reads as before.

