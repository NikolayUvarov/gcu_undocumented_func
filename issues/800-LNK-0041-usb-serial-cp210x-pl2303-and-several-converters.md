# 800-LNK-0041 — `usb_serial`: the CP210x and the PL2303, and several converters at once, each to its own client

**Type:** driver (USB class) · **Owner:** `LNK` · **Priority:** P1 · **Status:** open · **Blocked by:** for `init`'s part, a `KRN` task (requested when the driver's part is ready) · **Main task:** [800](800-mind-link.md); [100004](100004-two-allwinner-boards-before-the-phone.md) · **Constitution:** MC-2.3, MC-6.1, MC-12.4

## Problem

**The maintainer (2026-11-10):** the Allwinner boards are plugged into the DEXP, the Ryzen or the MacBook over USB, and are reached while those run MIND Core too.

**What `usb_serial` does today** ([800-LNK-0002](800-LNK-0002-usb-serial-ftdi-and-ch340.md)):
- it drives only the FTDI FT232R and the WCH CH340;
- it takes the first converter `usb_host` gives it;
- it serves that converter to one client, `link`.

**What the boards need of a MIND Core host:**
- **The usual 3.3 V USB-UART adapters:** many carry a Silicon Labs CP2102 or CP2104, or a Prolific PL2303, beside the FTDI and the CH340.
- **Two converters at once:**
  - the host's own RS-485 line, on which it is `link`'s slave to the Windows machine;
  - the board's adapter, which it masters ([800-LNK-0042](800-LNK-0042-mind-core-as-the-master-of-a-serial-line.md)).
- **Each converter told apart** by its identity (vendor, product, serial number, the port's path), so that `link.conf` and `serialhost`'s settings name which is which.

## Plan

1. **The CP210x** (AN571: vendor requests `IFC_ENABLE`, `SET_BAUDRATE`, `SET_LINE_CTL`, `SET_MHS`, `SET_BREAK`) and **the PL2303** (its vendor init sequence and CDC's `SET_LINE_CODING`, `SET_CONTROL_LINE_STATE`, `SEND_BREAK`), in `usb_serial/src/cp210x.rs` and `pl2303.rs`, as `ftdi.rs` and `ch340.rs` are.
   - Their IDs join `mind::usb::SERIAL_DEVICES`. That is libmind's core, so it goes with `KRN`'s part.
2. **A break and the control lines** in `serial.wit`, for U-Boot's console and the master's console mode: `break(ms)`, DTR and RTS set.
   - That is a new minor version, `serial.wit` 1.2 (MC-12.4).
   - `link` itself never uses them on its line (`LNK → EFF`, 2026-11-09: no break, DTR or RTS on a link line).
3. **Several converters:**
   - one `usb_serial` instance per converter, or one instance serving several;
   - each with its identity in `info`;
   - each lent to the client its settings name;
   - which form, with `init`'s part, is settled with `KRN`.
4. **Tests:**
   - host tests of each chip's divisor or line coding;
   - QEMU's `usb-serial` (an FTDI) for the several-converter case;
   - on hardware, each chip at hand, at 115200 and 1,000,000.

## Acceptance criteria

- **A CP2102 and a PL2303** each carry `mindlink` put and get of 64 KiB, compared, at 115200, on a MIND Core host. The chips at hand are named (MC-12.1).
- **Two converters on one host:**
  - `link` on one answers the Windows machine's effector485;
  - `serialhost` reads the board's console on the other.

## Related

[800-LNK-0002](800-LNK-0002-usb-serial-ftdi-and-ch340.md), [800-LNK-0042](800-LNK-0042-mind-core-as-the-master-of-a-serial-line.md), [100004-MOB-0004](100004-MOB-0004-effector-to-the-boards-over-their-uart.md), [docs/remote-serial.md](../docs/remote-serial.md), section 4.6.
