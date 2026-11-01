# 213-KRN-0099 — A `usb_host` for every xHCI controller

**Type:** `init`, with the drivers track's `usb_host` and class drivers · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** — (the drivers track's half: [requests-DRV.md](requests-DRV.md)) · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-2.3

## Problem

`init` starts one `usb_host`, for the first xHCI controller. The Ryzen 9 5900HX machine has two (05:00.3 and 05:00.4, 1022:1639), and what is plugged into the second's ports is not seen: its bus mastering stays off.

## Plan

1. **Instances:** `usb_host#1` in `SERVICE_INSTANCES`, given the second xHCI as `virtio_net#1` is given the second card, with its own DMA region and interrupt.
2. **Class drivers:** `usb_storage` and `usb_hid` take a client of each `usb_host` that runs (their slots chosen with the drivers track).
3. **Tests:** a QEMU suite with two `qemu-xhci` controllers, a keyboard on the second.

## Acceptance criteria

- In QEMU a keyboard on the second controller types in the shell.
- On the Ryzen machine both controllers are granted to a driver in the hardware report.

## Related

[213](213-amd-ryzen-pc.md), 164 (`usb_host`), [requests-DRV.md](requests-DRV.md).
