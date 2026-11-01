# 213-KRN-0099 — A `usb_host` for every xHCI controller

**Type:** `init`, with the drivers track's `usb_host` and class drivers · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress (made; QEMU on the machines, then the Ryzen machine) · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-2.3

## Problem

`init` starts one `usb_host`, for the first xHCI controller. The Ryzen 9 5900HX machine has two (05:00.3 and 05:00.4, 1022:1639), and what is plugged into the second's ports is not seen: its bus mastering stays off.

## Plan

1. **Instances:** `usb_host#1` in `SERVICE_INSTANCES`, given the second xHCI as `virtio_net#1` is given the second card, with its own DMA region and interrupt.
2. **Class drivers:** `usb_storage` and `usb_hid` take a client of each `usb_host` that runs (their slots chosen with the drivers track).
3. **Tests:** a QEMU suite with two `qemu-xhci` controllers, a keyboard on the second.

## Acceptance criteria

- In QEMU a keyboard on the second controller types in the shell.
- On the Ryzen machine both controllers are granted to a driver in the hardware report.

## Progress (2026-11-01)

- **No change to the drivers.** Each class driver takes one `usb_host` client in `SLOT_DEV0`, and `usb_host` polls its controller and skips the EHCI slots it was not given. So a second controller needs instances, not drivers that hold several clients: the drivers track's half asked for in requests-DRV.md is not needed (that item is taken out).
- **`SERVICE_INSTANCES`** (`common/abi.rs`; init's list, no ABI change, as `rtl8168#1` was) holds `usb_host#1`, `usb_storage#1` and `usb_hid#1`. Boot order puts each instance right after its image: `usb_host`, `usb_host#1`, `usb_storage`, `usb_storage#1`, `usb_hid`, `usb_hid#1`.
- **`init`:**
  - `usb_host`'s instance n takes the n-th xHCI controller in PCI order, its BAR0 and its own DMA region; the EHCI controllers go with the first only;
  - `usb_storage`'s and `usb_hid`'s instance n take the badged client of `usb_host`'s instance n, and start only while it runs;
  - `vfs_server` takes `usb_storage#1` as it takes `usb_storage`;
  - `usb_serial`, `bt_hci` and `video_gw`'s camera client stay the first controller's; the shell's keyboard client stays the first `usb_hid`'s.
- **Tests:**
  - the host test `each_usb_host_starts_before_the_class_drivers_on_it` (`tests/services_host.rs`);
  - `second_xhci_check` in the `usb` suite, on x86 and aarch64: a tablet on the first `qemu-xhci`, a keyboard on the second. The keyboard's keys reach `keys` through `usb_hid#1`, and the first `usb_hid` sees only the tablet;
  - the harness counts the three instances as services.
- **Docs:** README's service rows, `docs/profile/bootstrap.md`.

## Related

[213](213-amd-ryzen-pc.md), 164 (`usb_host`), [requests-DRV.md](requests-DRV.md).
