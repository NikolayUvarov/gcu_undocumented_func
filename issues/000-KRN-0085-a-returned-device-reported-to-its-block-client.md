# 000-KRN-0085 — A block device that came back is reported to its client, which checks the medium first

**Type:** kernel and services (`libmind/src/block.rs`, `block_protocol.rs`, `vfs_server`) · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** the request in [requests-DRV.md](requests-DRV.md) ("`usb_storage` reports a device that came back") · **Roadmap:** track B · **Constitution:** MC-4 (data integrity), MC-6.3, MC-6.4

## Problem

[000-KRN-0079](../issues-done/000-KRN-0079-a-returning-drive-is-checked-to-be-the-same-medium.done) checks the medium when the drive answers again after `vfs_server` saw it not answer. If `vfs_server` asks nothing while the stick is out, it never sees that.

`usb_storage` then claims the new stick inside the next request and runs that request on it. A write of the old system's cached sectors could reach another medium before anything is checked. In QEMU, `tests/usb_image_smoke.py` shows the check only once a `run` of a program makes `vfs_server` see the drive gone.

## Plan

1. **Libmind, the driver side.**
   - `block_protocol::Driver` gets `fn returned(&mut self) -> bool { false }`.
   - `block::serve` reads it before each read, write and flush. After a return it drops the client's buffer and answers "not found", as a restarted driver does.
2. **Libmind, the client side.**
   - `block::Device` keeps re-attaching and repeating a request after "not found" or "peer", for the clients that need nothing more (MC-6.4).
   - A client that asks for it (`Device::check_returns`) gets an error marked as a return instead, and re-attaches when it calls `reattach()`.
3. **`vfs_server`.** On a return it re-attaches and runs the medium check of 000-KRN-0079:
   - the same medium: the request is repeated;
   - another one: the volumes close.

   A restarted driver is checked the same way.
4. **`usb_storage`** (the drivers track, by request): it reports the return instead of running the command.
5. **The windows (added 2026-11-10, for 307-KRN-0177).**
   - **The gap.** A device that comes back keeps the driver instance's windows (307-DRV-0054), since its driver did not restart. So the block store's window 1 reaches whatever medium is now in the stick's place, by sector numbers.
   - **The fix:**
     - `block::serve` clears every window on a return, as a restart does. A window's client then gets "not found" until the window is set again;
     - `init` sets the store's window again only after the check it makes after a restart: the same partition number, PARTUUID and sectors, still the store's candidate (`store_window_again`, 307-KRN-0177).
   - **How `init` learns of a return** is part of this task's design. One way is the driver restarting itself on a return, so that `init`'s restart path runs.

## Acceptance criteria

- In QEMU, the stick is swapped for a copy with another manifest while `vfs_server` makes no request:
  - nothing is written to the copy;
  - `vfs_server` logs `CAME BACK WITH ANOTHER MEDIUM`.
- The same stick plugged in again still works.
- A driver killed and restarted is checked and goes on.
- The block store's window does not reach the copy: after the swap its client gets "not found", and `init` sets no window on the copy unless its partition passes the check.

## Related

[000-KRN-0079](../issues-done/000-KRN-0079-a-returning-drive-is-checked-to-be-the-same-medium.done), [211-KRN-0050](211-KRN-0050-a-program-on-an-unplugged-disk-is-refused-at-once.md), [211-DRV-0019](../issues-done/211-DRV-0019-usb-storage-refusals-and-resets.done).
