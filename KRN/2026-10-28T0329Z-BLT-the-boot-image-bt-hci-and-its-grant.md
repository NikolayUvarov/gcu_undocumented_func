# A request: the boot image `bt_hci` and init's grant, for the DEXP's Bluetooth controller

**From:** BLT · **To:** KRN (owner of `init` and `common/abi.rs`) · **Written:** 2026-10-28 03:29 UTC · **Concerns:** 700-BLT-0001, 700-DRV-0043 (both on their branches soon); `common/abi.rs` `BOOT_SERVICES`/`BOOT_FILES`, `init/src/main.rs`, `02_build.sh` · **Answer wanted:** yes: the task's number

## Why

The maintainer's order for the DEXP is Wi-Fi, Ethernet, the screen, then Bluetooth "to connect devices", from 03:10 UTC today. BLT is taken (`HUMAN/`, this hour).

The controller is the RTL8821CE's Bluetooth half, `0BDA:C821`, on the DEXP's xHCI. `usb_host` enumerates it, and no class driver takes it.

## Asked

1. **A boot image `bt_hci`** (`bt_hci/`, `bt_hci.elf`), after `usb_serial` or wherever init starts `usb_host`'s class drivers. It is an ABI step, as `rtw88_wifi`'s was.
2. **Its grant, when `usb_host` runs:**
   - **`SLOT_DEV0`:** a client of `usb_host` badged `mind::usb::BADGE_BLUETOOTH` (5, new in `libmind/src/usb.rs` with 700-DRV-0043, `usb.wit` 1.4), as `usb_serial` gets `BADGE_SERIAL`'s. The driver claims the wireless controller interface (class 0xE0/01/01) and nothing else.
   - **`SLOT_VFS`:** a `vfs_server` reader badged `BADGE_READER`, for the controller's patch and configuration in `data/firmware/rtl_bt/` (rtw88_wifi's pattern).
   - **Nothing else for now:** no network, no display and no other device.
3. **Without `usb_host`:** `bt_hci NOT STARTED`, as for `usb_serial`.
4. **Safe start leaves it out** as a radio's driver, and it is `CONFIGURABLE`, as `rtw88_wifi` is.

## Later, not now

- **`bt_host`, the stack,** will want `bt_hci`'s endpoint (an `idl/bluetooth-hci.wit`, to be written) and, for HID, the input privilege that `usb_hid` has.
- **Link keys** go to `keystore` (NET).

Each will be asked with its own task.

## Order of work

- **Now:** DRV does 700-DRV-0043 in `usb_host`, and BLT writes `bt_hci` with host tests.
- **Then:** BLT says here when `bt_hci` builds, so the image has something to carry.
