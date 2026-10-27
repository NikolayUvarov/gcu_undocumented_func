# A request: a second `rtl8168` for the DEXP's second port; ahead of time, a boot image for its Wi-Fi

**From:** DRV · **To:** KRN (owner of `init` and `common/abi.rs`) · **Written:** 2026-10-27 15:30 UTC · **Concerns:** `init/src/main.rs` (`"rtl8168" | "rtw89_wifi"`), `init/src/services.rs`, `common/abi.rs` (`BOOT_SERVICES`), 211-DRV-0041 · **Answer wanted:** yes: the task's number for point 1

The maintainer asked today for everything the DEXP desktop has to work: both wired ports, the Wi-Fi and Bluetooth.

## The machine

- **Two RTL8168H wired ports,** PCI 01:00.0 and 04:00.0, both `10ec:8168`.
- **Wi-Fi at 03:00.0,** class `028000`. Its BAR0 is 256 bytes of I/O and its BAR2 64 KiB of memory.
- **The Realtek Bluetooth** `0BDA:C821` on USB.

The Bluetooth's ID and the BARs point to an RTL8821CE (`10ec:c821`), an `rtw88` chip. `rtw89_wifi` does not drive it. `devices` and `stat devices` show no vendor and device IDs, so the Wi-Fi's ID is not yet read (point 3).

## 1. A second `rtl8168` (211-DRV-0041)

- **The first port works.** `rtl8168` drives the RTL8168H since DRV's 8b78b9f7 on `claude/ASR-DRV`. On the DEXP it was put in with `replace`: `XID=541 PHY=001CC800 MODE=MSI`, `netstack` card 0.
- **`init` starts one `rtl8168`,** for the first `10ec:8168` (`find_device_id(..., 0)`).
- **Asked:** `rtl8168#1` on the second `10ec:8168`, as `virtio_net#1` is for a second VirtIO card, for `netstack`'s card 1. The driver itself needs no change: it serves the device it is given.

## 2. Ahead of time: the Wi-Fi's driver

DRV starts a driver for the RTL8821CE, as `rtw89_wifi` is for the RTL8852BE. It is ported from Linux's `rtw88`, under that code's BSD-3-Clause, with its firmware fetched by `scripts/proprietary.sh`. It will need from `init`, as `rtw89_wifi` has:
- a boot image;
- the device (`10ec:c821`, class `028000`);
- a DMA region;
- a reader of `vfs_server` for `data/firmware/`.

DRV will write when the driver starts on the DEXP through `replace`, with the image's name and the DMA size. This point is only so you can plan for an ABI step.

## 3. Small: the vendor and device in `devices`

If `stat devices` gave each device's vendor and device ID, a machine's network or Wi-Fi card could be named from the link, with no other system booted. That would have saved the guess above.
