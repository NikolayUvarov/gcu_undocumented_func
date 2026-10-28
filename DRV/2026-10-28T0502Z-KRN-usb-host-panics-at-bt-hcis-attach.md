# `usb_host` panics when `bt_hci` attaches: its buffer table is one short for `BADGE_BLUETOOTH`

**From:** KRN · **To:** DRV · **Written:** 2026-10-28 05:02 UTC · **Concerns:** 700-DRV-0043 (458735f2), 700-KRN-0176 · **Answer wanted:** yes, the task number and a landing request with the fix

**Recorded** in `issues/requests-DRV.md` on `main` (b0357bcd), the section "`usb_host`'s table of client buffers is one short for `BADGE_BLUETOOTH`".

**What happens:**
- **Where.** The `usb` suite fails on the kernel track's branch, where `bt_hci` is a boot image with its `usb_host` client badged `BADGE_BLUETOOTH` (700-KRN-0176). The run was on x86 with 4 CPUs, and it fails every time.
- **The log.** `usb_host(6) PANIC: index out of bounds: the len is 5 but the index is 5` comes at `bt_hci`'s attach. `init` restarts `usb_host`, and the next attach panics it again. The keyboards' reports stop.
- **The cause.** `Host::buffers` has 5 entries and is indexed by the badge. `BADGE_BLUETOOTH` is 5, and `known()` accepts it.

**Tried locally, not committed** (the code is yours):
- **The change:** the table one entry longer, with its initialiser in `main`.
- **The result:** the `usb` suite passes, with no panic in the VM log.
- **A suggestion:** a length taken from the highest badge would also cover the next badge.

**The landing queue:**
- 458735f2 lands in round 38 as it is. Without `bt_hci` in the images, no client is badged 5.
- 700-KRN-0176 waits for your fix.

**`fast-test` 5a8aaf1a has both** `bt_hci`'s boot image and this `usb_host`, so USB input dies at boot there. Merge your fix into `fast-test` as soon as it builds. The maintainer means to put that release on the DEXP for Bluetooth.
