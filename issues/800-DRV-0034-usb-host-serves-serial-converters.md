# 800-DRV-0034 — `usb_host` serves USB-serial converters: claimed by chip, read a packet at a time

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 · **Status:** in progress · **Blocked by:** — · **Main task:** [800](800-mind-link.md), for [800-LNK-0002](800-LNK-0002-usb-serial-ftdi-and-ch340.md) · **Constitution:** MC-2.3, MC-3.3, MC-12.4

## Problem

`usb_serial` (800-LNK-0002) drives the FTDI FT232R and the WCH CH340. `usb_host` cannot serve it as it stands:
- **Claiming:** it hands interfaces to class drivers by interface class. These chips' only interface is vendor-specific (class 0xFF), which no badge names.
- **Setting up:** the chips take vendor requests addressed to the device as a whole, and `usb_host` refuses every request to the device but reading a descriptor.
- **Reading:** a bulk IN transfer waits until data comes, spinning for up to 30 s. A converter's line is idle most of the time. The CH340 answers nothing while it has no data, and the FTDI chip answers every 1 to 16 ms with its two status bytes, so `usb_host` would spin a CPU while a converter is attached.

## Plan

- **A badge for the converters:** `mind::usb::BADGE_SERIAL` (4). It claims a vendor-specific interface of a chip `mind::usb::SERIAL_DEVICES` names, by vendor and product ID. `known` and `claims` replace the class check.
- **Vendor requests to the device** are allowed when it is such a converter: it has no other interface to protect.
- **Reading without blocking:** `reports` and `reports-up-to` also take a bulk IN endpoint of up to 64-byte packets, on xHCI. `usb_host` keeps eight transfers of a packet each on its ring, as for an interrupt endpoint, and queues what comes for the client.
- **`idl/usb.wit` 1.3** says so; the wire is unchanged.

## Acceptance criteria

- **In QEMU,** `usb_serial` claims the emulated FT232 (`-device usb-serial`) under `BADGE_SERIAL`, sets it up and moves 1 MiB each way through it (800-LNK-0002's run).
- **The other class drivers** still claim only their own class: the USB suites pass.

## Related

[800-LNK-0002](800-LNK-0002-usb-serial-ftdi-and-ch340.md), `usb_host/src/main.rs`, `libmind/src/usb.rs`.
