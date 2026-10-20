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

## Progress

**2026-10-20, a local QEMU run, before the gate.**
- **The setup:** QEMU's emulated FT232 (`-device usb-serial` on `qemu-xhci`, its line on a pseudo-terminal), with `init` patched locally to start `usb_serial` and `link`. That patch is KRN's to make and is not committed.
- **The run:** `usb_host` gave the converter to `usb_serial` under `BADGE_SERIAL`, took its setup requests and polled its bulk IN endpoint. 1 MiB went each way intact (800-LNK-0002, 0003).
- **Two faults the run found, both fixed:**
  - **A converter's endpoints got no rings.** They are built only for the interfaces of served classes, so `reports` refused the endpoint. A converter's interface now counts as served.
  - **The bulk IN ring held its own report buffers over its transfers.** A bulk ring fills its page, and the buffers sit at 0x400, so the endpoint stalled after 64 transfers. A converter's bulk IN endpoint now gets the polling ring, 64 transfers with the buffers after them, and `arm` refuses any other ring.
- **Flow control:** a bulk IN endpoint is now lossless. A transfer goes back on the ring only while the 8-report queue has room for its packet, so the device holds what does not fit (NAK) instead of the oldest report being dropped. On emulated xHCI, re-armed transfers complete at once, and the old drop-oldest queue lost the middle of 600-byte frames.
- **Left:** the USB suites in the gate, and a run on real hardware.

## Related

[800-LNK-0002](800-LNK-0002-usb-serial-ftdi-and-ch340.md), `usb_host/src/main.rs`, `libmind/src/usb.rs`.
