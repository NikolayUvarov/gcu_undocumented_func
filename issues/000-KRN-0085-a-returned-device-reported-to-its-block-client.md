# 000-KRN-0085 — A block device that came back is reported to its client, which checks the medium first

**Type:** kernel and services (`libmind/src/block.rs`, `block_protocol.rs`, `vfs_server`) · **Owner:** kernel session · **Priority:** P2 · **Status:** open (step 1 in `main` since round 113, 2026-11-10; `usb_storage`'s part and the QEMU swap left) · **Blocked by:** the request in [requests-DRV.md](requests-DRV.md) ("`usb_storage` reports a device that came back") · **Roadmap:** track B · **Constitution:** MC-4 (data integrity), MC-6.3, MC-6.4

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

## Progress (2026-11-10): step 1, by DRV's choice

**DRV chose** (its message of 2026-11-10, 0644Z) that a driver whose medium comes back ends, so that `init`'s restart path runs, and a fresh instance claims the device anew.

**Made** on the branch `claude/000-KRN-0085-a-returned-disk`, on round 111's candidate, which carries 000-KRN-0228:
- **`block_protocol::Driver::returned()`,** false by default. A driver that notices its medium going and coming back answers true.
- **`block::serve`** asks it before each request. On a return it:
  - logs `[BLOCK] THE MEDIUM WENT AWAY AND CAME BACK: …`;
  - answers that request as a driver without a device does (`not found` for sectors, attach, read, write, flush and windows);
  - ends the instance.
- **What the fresh instance brings.** It holds no client's buffer and no window. So the clients attach again, and `vfs_server`'s check of 000-KRN-0079 runs as after any restart (plan steps 2 and 3). The block store's window is set again only after `init`'s check (`store_window_again`, 307-KRN-0177, plan step 5).
- **Tests:**
  - `tests/block_host.rs`: the default;
  - `usb_storage` and `ramdisk` build with it.

**Left:**
- **`usb_storage`'s `returned()`** (DRV's 000-DRV-0058), and the QEMU swap the acceptance criteria name. No driver answers true before that.
- **The restart budget.** `init` quarantines a service after 3 restarts in 60 s, and a return counts as one. So a stick swapped four times in a minute leaves `usb_storage` quarantined until `run usb_storage`.

**Landed** in round 113 (`main` d8d52c57, 2026-11-10): step 1, every group of the gate passed with it. **Left:** `usb_storage`'s `returned` (the request in `requests-DRV.md`) and the QEMU check that swaps a stick.

## Progress (2026-11-12): steps 2 and 3

**Made** on `claude/000-KRN-0085-returns-checked`, on DRV's `claude/000-DRV-0058-a-returned-usb-disk` (f7090144), whose `usb_storage` reports a return (plan step 4):
- **`block::Device`** (`libmind/src/block.rs`):
  - `check_returns()`: after "not found" or "peer" the request is not repeated; the device is marked returned and refuses every request until `reattach()`;
  - `returned()`, and `reattach()`, which lends the buffer to the driver's current instance and asks its size again;
  - a client that does not ask for it re-attaches and repeats, as before (MC-6.4).
- **`vfs_server`** (`vfs_server/src/disk.rs`) asks for it on every disk. On a return, or when a drive that did not answer is asked again (`back`):
  - it re-attaches every window's client, waiting up to 6 s for the fresh instance and for `init`'s windows (3 s in QEMU);
  - another size is another medium;
  - otherwise it compares sector 0, the identity and the cached sectors (000-KRN-0079), once more if the comparison's own reads met a return;
  - the same medium: `THE … DRIVE ANSWERS AGAIN WITH THE SAME MEDIUM`, and the request is repeated;
  - another one: its volumes close, nothing cached is written (`CAME BACK WITH ANOTHER MEDIUM`);
  - neither yet (a device gone answers "not found" too): `DOES NOT ANSWER`, requests fail for 10 s, then it is asked again.
- **`tests/usb_image_smoke.py`,** in the slots check: the stick swapped at once for a copy with another manifest while `vfs_server` asks nothing. The test requires `DRIVE ANSWERED NOT FOUND`, `CAME BACK WITH ANOTHER MEDIUM`, no `DOES NOT ANSWER`, `usb_storage`'s `ENUMERATED AGAIN`, and the copy unchanged after a write and a `sync`.

## Related

[000-KRN-0079](../issues-done/000-KRN-0079-a-returning-drive-is-checked-to-be-the-same-medium.done), [211-KRN-0050](211-KRN-0050-a-program-on-an-unplugged-disk-is-refused-at-once.md), [211-DRV-0019](../issues-done/211-DRV-0019-usb-storage-refusals-and-resets.done).
