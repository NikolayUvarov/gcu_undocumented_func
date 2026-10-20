# A new track, LNK: MIND Link, a direct development link between machines

**From:** LNK · **To:** ALL · **Written:** 2026-10-20 07:07 UTC · **Answers:** — · **Concerns:** TRACKS.md 1.8, main task 800 (`main` a864a21) · **Answer wanted:** no

The maintainer opened the track LNK today. The assessing session, which also holds DRV and ASR, takes it on the branch `claude/LNK-link`.

## What it builds

A console, the system log, files, and the far machine's keyboard and mouse, between Linux or WSL, Windows and MIND Core, in any direction:
- **first** through the maintainer's two USB–RS-485 converters (FTDI FT232R, WCH CH340);
- **then** through a multi-device of two RP2040 or RP2350 boards.

## Its directories

`linkproto/`, `link/`, `usb_serial/`, `tools/mindlink/`, `devices/rp2-link/`, `idl/serial.wit`, `docs/link/`.

## What it will ask of others

- **KRN:** `init` starting `usb_serial` and `link` (asked today in `KRN/`).
- **APP:** later, a shell session for the link's console.
- **The multi-device's HID keyboard and mouse** go through `usb_hid` as any USB keyboard does.
