# 800 — MIND Link: a direct development link between machines

**Type:** main task · **Owner:** `LNK` · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Roadmap:** the development of MIND Core on real hardware (tracks A and D's evidence on machines) · **Constitution:** MC-2.3, MC-2.4, MC-6.1, MC-11.4, MC-12.1

## Problem

Developing MIND Core on a real machine means moving files and logs by hand: a USB disk carried between the machines, read in WSL, written and carried back. The maintainer asked (2026-10-20) for a direct link between the development machine and the machine under test:
- a console and the system log;
- files moved quickly in both directions;
- the remote machine's keyboard and mouse driven from the other end.

It must work in any direction: Linux or WSL to MIND Core, Linux to Windows, MIND Core to Linux, Windows or MIND Core.

## Plan

Two transports, one protocol:

1. **First, a serial line through the maintainer's two USB–RS-485 converters** (a WCH CH340, 1A86:7523, and an FTDI FT232R, 0403:6001). It is half-duplex and about 0.2–0.3 MB/s, for consoles, logs and files.
   - [800-LNK-0001](../issues-done/800-LNK-0001-the-link-protocol.done) the protocol;
   - [800-LNK-0002](800-LNK-0002-usb-serial-ftdi-and-ch340.md) MIND Core's driver for the converters;
   - [800-LNK-0003](800-LNK-0003-the-link-service-on-mind-core.md) MIND Core's end;
   - [800-LNK-0004](800-LNK-0004-mindlink-the-host-tool.md) the Linux and WSL end.
2. **Then the multi-device:** two RP2040 or RP2350 boards joined back to back, each a USB device to its own machine.
   - Each offers a serial port (CDC-ACM), a fast channel (vendor bulk, about 1 MB/s each way, USB full speed's limit), and a keyboard and mouse (HID) that the other end drives.
   - Its RGB LED shows the link's state and the machine's boot.
   - [800-LNK-0005](800-LNK-0005-the-rp2-multi-device-firmware.md) the firmware;
   - [800-LNK-0006](800-LNK-0006-mind-core-hosts-the-multi-device.md) MIND Core's side;
   - [800-LNK-0007](800-LNK-0007-the-windows-end.md) Windows.
3. **MIND Core as the master,** so it reaches Linux, Windows or another MIND Core too: [800-LNK-0008](800-LNK-0008-mind-core-as-the-master.md).
4. **Work handed over the link without a person at the machine** (the maintainer's request, 2026-10-20):
   - [800-LNK-0009](../issues-done/800-LNK-0009-programs-started-for-the-master.done) programs started for the master, and their output;
   - [800-LNK-0010](800-LNK-0010-directories-in-data-link.md) directories below `data/link/`;
   - [800-LNK-0011](800-LNK-0011-effector485.md) `effector485`, a small Effector over the link;
   - [800-LNK-0012](../issues-done/800-LNK-0012-the-developer-mode.done) the developer mode, `dev` in `link.conf`.

The protocol runs between the two machines' ends. The converters and the boards only carry its bytes.

## Acceptance criteria

- **From WSL, through the RS-485 converters,** the maintainer reads a MIND Core machine's log and moves a file each way, checked by SHA-256, with no USB disk carried.
- **Through the multi-device,** the same at about 1 MB/s. The remote machine's pointer and keys are driven from the other end, in each direction the plan names.
- **In QEMU,** the link runs over QEMU's emulated FTDI (`-device usb-serial`) on every gate.

## Related

[issues-human/README.md](../issues-human/README.md) section 8 (the RP2040 indicator, whose protocol the multi-device keeps), `usb_host`, `usb_hid`, [750](750-effector-agent.md) (Effector, which may use the link later).
