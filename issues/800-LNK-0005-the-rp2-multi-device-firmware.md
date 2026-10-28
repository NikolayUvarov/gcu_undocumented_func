# 800-LNK-0005 — The multi-device: firmware for two RP2040 or RP2350 boards joined back to back

**Type:** firmware · **Owner:** `LNK` · **Priority:** P2 · **Status:** in progress · **Blocked by:** — (the boards' pins named by the maintainer) · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-6.1, MC-12.1

## Problem

Two machines cannot talk over USB directly, since both are hosts. The multi-device is two small boards wired to each other. Each is a USB device to its own machine, and together they carry:
- a serial port;
- a fast channel for files;
- a keyboard and a mouse that the far end drives.

The maintainer has RP2040 boards (the YD-RP2040 and RP2040-Zero kinds his `rasp2040-wake` firmware builds for) and an RP2350.

## Plan

- **`devices/rp2-link/`, C with the Pico SDK and TinyUSB,** as the maintainer's `rasp2040-wake` is. It builds for the RP2040 and the RP2350 from one source (`PICO_BOARD`, a board file per kind) to a UF2, by one documented command.
- **USB, a composite device:**
  - CDC-ACM, the serial port;
  - a vendor bulk interface, the fast channel, with Microsoft OS 2.0 descriptors so Windows binds WinUSB with no driver installed;
  - HID keyboard and HID mouse.

  The product string is `MIND Link`, and its serial number names the board.
- **Between the boards:** a full-duplex link of their own, on PIO or SPI at about 30 Mbit/s, with framing and credit-based flow control, so neither board's buffers overflow. Its wires are named in the board files.
- **What each board does:**
  - bytes from its host's serial port and fast channel go to the other board's matching interface;
  - input events (`linkproto` channel 4) that the far end sends come out of its HID keyboard and mouse to its own machine.
- **The RGB LED:**
  - powered;
  - its host connected;
  - the other board connected;
  - data flowing;
  - an error.

  It also shows the boot stages that MIND Core sends (`@STATE`, `@HEALTH`, `@BEAT`, issues-human section 8).
- **Third-party parts** (the Pico SDK, TinyUSB) in `THIRD_PARTY.md`.

## Acceptance criteria

- **One command** builds UF2 files for the RP2040 and the RP2350.
- **Two boards on two Linux machines** move 100 MiB through the fast channel at 0.9 MB/s or more each way, checked by SHA-256.
- **Keys and pointer moves** sent from one machine reach the other as a USB keyboard and mouse.
- **Each LED state** is shown by a test script.

## Progress

**2026-10-20, on `claude/800-LNK-0005-rp2-firmware`.** The firmware is written and builds; it has run on no board.

**Changes to the plan, and why:**
- **Three CDC-ACM ports in place of a vendor bulk interface:** serial, data and control. Every system the plan names has a CDC-ACM driver:
  - Windows 10 and later bind `usbser` with no descriptor of Microsoft's;
  - `mindlink` drives each port as a serial port, with no libusb;
  - MIND Core needs one class driver (800-LNK-0006).

  At full speed a CDC port's bulk endpoints are as fast as a vendor interface's.
- **The control port** takes text commands. Input commands are played on the other board's boot keyboard, boot mouse and absolute tablet. That serves issues-human section 10's test rig too, which asks for boot protocol and an absolute pointer.
- **The link between the boards:** a PIO UART at 15 Mbit/s on two wires, not SPI.
  - Both boards run the same firmware, with no master and no clock line.
  - At 120 MHz the rate divides evenly.
  - Frames are COBS-encoded with CRC-16, and credits carry absolute offsets.

**What is built:**
- **`devices/rp2-link/build.sh`** builds UF2 files for the RP2040-Zero, YD-RP2040, Pico, RP2350-Zero and Pico 2, after the host tests.
- **The LED** shows the link, or the indicator's `@STATE`/`@HEALTH`/`@BEAT` lines of section 8.
- **`mindlink board <line>`** sends one line to a control port.
- **[docs/link/multi-device.md](../docs/link/multi-device.md)** (Russian [multi-device_RU.md](../docs/link/multi-device_RU.md)) gives the wiring, the ports, the commands and the LED.
- **The Pico SDK and TinyUSB** are recorded in THIRD_PARTY.md.

**Host tests, `devices/rp2-link/tests/host_test.c`** (in `scripts/host_tests.sh`; also run under AddressSanitizer and UBSan by hand):
- COBS and the CRC;
- every damaged byte of a frame caught;
- noise and an overlong run;
- credits across the wrap;
- a stream through a line losing 3 % of frames that never overflows the window;
- every control command and key name;
- the LED's table.

**Left:**
- every acceptance criterion: they need the maintainer's boards, wired as the page says;
- `mindlink` keeping several requests in flight, so files move at the ports' speed.

## Related

[800](800-mind-link.md), [800-LNK-0006](800-LNK-0006-mind-core-hosts-the-multi-device.md), [800-LNK-0007](800-LNK-0007-the-windows-end.md), issues-human section 8.
