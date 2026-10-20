# 800-LNK-0002 — `usb_serial`: the FTDI FT232R and WCH CH340 USB-serial converters on MIND Core

**Type:** driver · **Owner:** `LNK` (with `DRV` for `usb_host`) · **Priority:** P1 · **Status:** open · **Blocked by:** `KRN`: `init` starting `usb_serial` with a badged `usb_host` client (asked on `internal-dev-messages`) · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-2.3, MC-2.4, MC-6.1

## Problem

The maintainer's two RS-485 converters are vendor-specific USB-serial chips, not CDC-ACM:
- the FTDI FT232R (0403:6001);
- the WCH CH340 (1A86:7523).

MIND Core has no driver for either, so neither can carry the link (800-LNK-0001) on a MIND Core machine.

## Plan

- **`usb_serial`, a ring-3 class driver of `usb_host`,** like `usb_storage` and `usb_hid`:
  - it claims the two chips by vendor and product ID (a table, so more can be added);
  - it sets the baud rate (to 3 Mbit/s on the FT232R, 2 Mbit/s on the CH340), 8N1 and the line state;
  - it moves bytes on their bulk endpoints.
- **`idl/serial.wit` 1.0:** `info` (the chip, the rate), `configure(baud)`, `write(bytes)`, `read() -> bytes`, `wait`. It is served to one client, which `init` names (MC-2.3).
- **`usb_host`** (the drivers track): a class driver may claim a vendor-specific interface by vendor and product ID, under its badge.
- **Facts only.** Register and request facts come from the vendors' published documents (FTDI's application notes, WCH's datasheet) and are described, not copied from GPL drivers.
- **Tests in QEMU.** QEMU's `-device usb-serial` emulates an FTDI FT232: the suite connects it to a host socket and moves bytes both ways at several rates. The CH340 gets host tests of its request sequences.

## Acceptance criteria

- **In QEMU,** `usb_serial` drives the emulated FT232, and 1 MiB goes each way intact through the host socket.
- **On the maintainer's machine,** each converter carries bytes to the other over RS-485: one plugged into a MIND Core machine, the other into WSL.

## Related

[800-LNK-0001](../issues-done/800-LNK-0001-the-link-protocol.done), [800-LNK-0003](800-LNK-0003-the-link-service-on-mind-core.md), `usb_host`, `usb_storage` (the model of a class driver).
