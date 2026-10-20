# 800-LNK-0002 — `usb_serial`: the FTDI FT232R and WCH CH340 USB-serial converters on MIND Core

**Type:** driver · **Owner:** `LNK` (with `DRV` for `usb_host`) · **Priority:** P1 · **Status:** in progress · **Blocked by:** `KRN`: `init` starting `usb_serial` with a badged `usb_host` client (asked on `internal-dev-messages`) · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-2.3, MC-2.4, MC-6.1

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

## Progress

**2026-10-20, on `claude/LNK-link`.**
- **`usb_serial` and `idl/serial.wit` 1.0** are written, and the FTDI FT232R is driven. Its setup comes from FTDI's application notes AN232B-04 and AN232B-05:
  - reset and both purges; 8N1, no flow control, DTR and RTS on; a 2 ms latency timer;
  - the rate's divisor: the nearer of the two around the exact one, refused beyond 3 %.
- **Host tests, `tests/serial_host.rs`:** the divisor against the nearest rate the chip can make, every 997 bit/s from 300 to 3.1 Mbit/s; and the two status bytes stripped. `serial.wit` is in the IDL fuzz.
- **The QEMU run** used the emulated FT232 with `init` patched locally to start `usb_serial` and `link` (KRN's part, not committed). Through `link` and `mindlink`, 1 MiB went each way intact, at 57 KB/s in and 127 KB/s out with 256-byte frames.
  - This is the acceptance's QEMU criterion met by hand, not yet a suite: the suite needs `init` to start the two services.
- **Found in the run, and fixed:**
  - a converter plugged in again was set to 115200 bit/s rather than the client's rate;
  - lost bytes and line errors were not logged. Both are now, and so is the reason a converter is taken as gone.
- **QEMU's limit:** the emulated chip keeps 384 bytes and is refilled only when QEMU's main loop wakes for something else, so larger frames wait for their tail. The run therefore used 256-byte frames (`mindlink --payload 256`, docs/link/mindlink.md).
- **2026-10-20: the CH340 driven too** (the maintainer's converters are CH340s), from the requests WCH's own serial driver uses:
  - version (`0x5F`), serial init (`0xA1`), DTR and RTS on (`0xA4`);
  - the rate registers `0x1312`, set by `configure`: the rate is 48 MHz / (2^(12 − 3·ps − fact) · divisor), and bit 7 sends a part-filled packet at once on chips after version `0x27`;
  - 8N1 in `0x2518`;
  - its bulk data have no status bytes.

  `tests/serial_host.rs` checks the rate registers against the rate they decode to: every 1013 bit/s from 50 to 3 Mbit/s, and the common rates. **Not yet run on a CH340 under MIND Core**: that waits for `init`'s part (800-KRN-0128).
- **Left:**
  - **`init` starting the service** (KRN);
  - **the run on the maintainer's converters.**

## Related

[800-LNK-0001](../issues-done/800-LNK-0001-the-link-protocol.done), [800-LNK-0003](800-LNK-0003-the-link-service-on-mind-core.md), `usb_host`, `usb_storage` (the model of a class driver).
