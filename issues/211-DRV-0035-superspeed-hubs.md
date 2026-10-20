# 211-DRV-0035 — `usb_host` walks a SuperSpeed hub's ports (a two-bay SATA dock on the MacBook Pro)

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 · **Status:** in progress · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-6.1, MC-12.1

Taken from `requests-DRV.md`, where the kernel track recorded it (2026-10-20, `internal-dev-messages` `DRV/2026-10-20T1834Z-KRN-…`).

## Problem

On the MacBook Pro, the boot disk in a two-bay SATA–USB dock plugged into a USB 3 port is not found:
- `vfs_server`: `THE BOOT VOLUME … IS ON NO BLOCK DEVICE: NONE MOUNTED`, and no program starts.
- The same disk on the same port through a single-bay adapter works, and the Mac's firmware boots from the dock.
- `usb_host`'s log: `05E3:0610 ON PORT 1 (HIGH SPEED) HUB` and `05E3:0626 ON PORT 5 (SUPER SPEED) HUB`. The dock is a Genesys Logic USB 3 hub, and nothing shows behind either half; `usb_storage` reports `NO MASS STORAGE DEVICE`.

**The cause.** `usb_host` walked only USB 2 hubs: `enumerate` skipped a hub at SuperSpeed. A USB 3 bridge behind a USB 3 hub links at SuperSpeed and never shows on the hub's USB 2 side, so the dock's SATA bridges were never set up. Nothing in the log said a hub was skipped.

## Plan

From USB 3.2 chapter 10:
- **Before its ports are used,** a SuperSpeed hub is told its depth (`SET_HUB_DEPTH`, request 12, 0 on a root port).
- **Its descriptor** is type `0x2A` (12 bytes).
- **Its slot context** marks it a hub with its port count, with no TT.
- **Its port status:**
  - power is bit 9, not low speed;
  - a device on its port is at SuperSpeed, with its route string as behind a USB 2 hub;
  - a port that does not come up from `PORT_RESET` gets a warm reset, `BH_PORT_RESET` (28), waited for by its own change bit.
- **Its other change bits** are cleared so the hub stops reporting the port: over-current, `C_BH_PORT_RESET` (29), `C_PORT_LINK_STATE` (25), `C_PORT_CONFIG_ERROR` (26).
- **A hub deeper than `usb_host` walks** is logged with its place.

## Acceptance criteria

- **On the MacBook Pro** the disk in the dock is found (`[USB] MASS STORAGE: … SECTORS`), the boot volume mounts and programs start. The same with the dock behind its USB 2 side (a USB 2 cable).
- **The QEMU USB suites** pass as before. QEMU has no SuperSpeed hub, so the hub's own path is checked on hardware: the dock on the MacBook Pro, with both bays, and the Ryzen's ports.

**A workaround meanwhile:** the single-bay adapter, or the dock on a USB 2 cable or behind a USB 2 hub, so that the bridges link at high speed behind the hub's USB 2 side.

## Progress

**2026-10-20, on `claude/211-DRV-0035-superspeed-hubs`.**
- **Written as planned.** The hub facts are in `usb_host/src/hub.rs`: the descriptors, the speed after a reset, and the change bits to clear.
- **Host-tested** by `tests/usb_hub_host.rs` (in `scripts/host_tests.sh`):
  - a Genesys Logic SuperSpeed hub's descriptor and its USB 2 side's;
  - a SuperSpeed port's powered bit not read as low speed;
  - the change bits of each kind of hub.
- **2026-10-20, the MacBook Pro, the first run (a trial image with `fast-test` 3e282a5c, read over MIND Link).**
  - **The SuperSpeed hub was walked:** `05E3:0626 ON PORT 5 (SUPER SPEED) HUB`, then `05E3:0749 ON PORT 5.3` and `152D:0562 ON PORT 5.4`, both at SuperSpeed. The second is the JMicron SATA bridge holding the boot disk.
  - **But `usb_storage` gave up on the first mass storage device,** `05E3:0749`, the dock's card reader without a card, and never reached the disk. So `vfs_server` found the boot volume on no block device.
  - **Fixed:** a device with no medium, or not ready, stays claimed, and the next one is tried. The log names the device taken. In QEMU, an empty removable USB drive on port 1 was skipped, and the boot disk on port 2 mounted with its log volume.
  - **Left:** the Mac's run with the fix.

## Related

`usb_host/src/main.rs` (`configure`, `reset_hub_port`, `scan`), [800-DRV-0034](800-DRV-0034-usb-host-serves-serial-converters.md).
