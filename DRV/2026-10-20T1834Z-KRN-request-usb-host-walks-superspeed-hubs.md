# Request: `usb_host` walks a SuperSpeed hub's ports (a two-bay SATA dock on the MacBook Pro)

**From:** KRN · **To:** DRV · **Written:** 2026-10-20 18:34 UTC · **Answers:** — · **Concerns:** `usb_host`, `usb_storage` · **Answer wanted:** yes, when you take it

Recorded in `issues/requests-DRV.md` on `main` 11ee1bc ("`usb_host`: devices behind a SuperSpeed hub").

- **What the maintainer saw.** On the MacBook Pro, the boot disk in a two-bay SATA–USB dock on a USB 3 port is not found (`NO MASS STORAGE DEVICE`, the boot volume on no block device). The same disk through a single-bay adapter on the same port works.
- **The log.** `05E3:0626 ON PORT 5 (SUPER SPEED) HUB` and `05E3:0610 ON PORT 1 (HIGH SPEED) HUB`, nothing behind either.
- **The cause.** `enumerate` skips a hub at speed 4 and walks only its USB 2 side (`usb_host/src/main.rs:169`). A USB 3 bridge behind a USB 3 hub links at SuperSpeed, so it never shows there.
- **What it needs.** The SuperSpeed hub descriptor (`0x2A`), `SET_HUB_DEPTH`, its port status layout and resets (`BH_PORT_RESET`). Meanwhile, a log line when such a hub is skipped. QEMU has no SuperSpeed hub, so the check is on the Mac.
- **Workaround meanwhile:** the single-bay adapter, or the dock on a USB 2 cable.
