# 100000-LNK-0038 — `link` over the board's console UART, so that `effector485` reaches the PinePhone

**Type:** service, tool · **Owner:** `LNK` · **Priority:** P1 · **Status:** in progress (2026-11-10) · **Blocked by:** — ([100000-KRN-0221](../issues-done/100000-KRN-0221-init-starts-uart-serial-for-link.done), `init`'s part, done); for the board runs, [100004-MOB-0002](100004-MOB-0002-the-first-boot-on-the-h5.md) and [0003](100004-MOB-0003-the-first-boot-on-the-h618.md) · **Main task:** [100000](100000-a-phone-the-pinephone-first.md), [800](800-mind-link.md) · **Constitution:** MC-2.3, MC-6.1, MC-12.3, MC-12.4

Numbered from the kernel session's request of 2026-11-08 in `requests-LNK.md`.

## Problem

**What the phone has.** The PinePhone's only line out is UART0, through the headphone jack, at 115200 8N1, and it is also the console. Its agent has no network until stage 3. So its updates, logs and tests go through `effector485` over that line, with the console as channel 1.

**What `link` has.** It takes its line only from `usb_serial`, an RS-485 or RS-232 converter on USB, through `idl/serial.wit` (`SLOT_DEV0`).

## Plan

1. **A serial service on the UART, `uart_serial`,** serving `serial.wit` as `usb_serial` does:
   - `link` keeps one kind of line and does not change;
   - the UART is driven in one place, not in each of its clients.

   Its parts:
   - **the chip:** `mind::dev::Uart`'s 16550 at ports (COM1, x86) and PL011 (QEMU's `virt`) first; the A64's 16550 in memory once 100000-KRN-0171 settles its page, or over the kernel's byte channel if that is the answer;
   - **receiving:** polled at first, with the line's interrupt once `init` lends it;
   - **what is kept:** a buffer of 4 KiB at least, so that a frame of up to 1 KiB arrives whole while `link` is busy.
2. **`serial.wit` 1.1:** the UART chips named in `info.chip`, and `configure` refused (`invalid`) for a rate the firmware's line was not set to, until the service sets the divisor itself. This is a new version (MC-12.4). `usb_serial` and `link` move to it in the same change.
3. **`link.conf`'s word `uart`:**
   - `init` (KRN) starts `uart_serial` with the console UART;
   - it lends `link` that client instead of `usb_serial`'s, and lends the shell no serial line;
   - without `baud=`, `link` stays at 115200.
4. **The console on that line:** `link-console` (800-LNK-0019). On a UART the shell cannot share the line, so channel 1 is the console.
5. **The boot's first lines,** sent before `link` answers: `effector485` and `mindlink` keep the bytes outside frames and show them as console text (800-LNK-0039).
6. **Tests in QEMU:**
   - x86 with COM1 on a pty;
   - `virt` under U-Boot with its PL011 on a pty;
   - in both, `mindlink` and `effector485` at 115200: `hello`, `put`, `get`, `log`, `shell`.

## Acceptance criteria

- The QEMU runs of plan step 6 pass in a suite of their own, x86 and aarch64.
- On the PinePhone, once stage 0 boots: `effector485 hello` and `log` over the headphone jack's UART at 115200.

## Progress (2026-11-09)

**The shape agreed** with the kernel session (`LNK/2026-11-09T0504Z-KRN-agreed-…`): `uart_serial` serves `serial.wit`, and `init`'s part is 100000-KRN-0221. On the A64 the line is the kernel's console channel, 100000-KRN-0222, in `main` since round 102.

**Made** on `claude/LNK-link` (plan steps 1 and 2):
- **`uart_serial`:** the line in `SLOT_DEV0` through `mind::dev::Uart`.
  - **What it drives:** a 16550 at ports (chip 3), a PL011 (chip 4), or the kernel's console channel (chip 5).
  - **Receiving:**
    - with a line in `SLOT_IRQ` (a 16550's or a PL011's), it is bound to the service endpoint and armed on the chip, and the chip is also read every 2 ms;
    - without one, every 2 ms;
    - the channel's bytes are read in pieces of 256 without waiting.
  - **The queue:** 64 KiB; bytes not taken in time and the chip's overruns are counted and logged.
  - **Writes:** a 16550 or a PL011 is sent to byte by byte, its receive FIFO emptied while it waits. The channel is written in whole calls, waiting 1 ms while the kernel's queue is full.
  - **The rate:**
    - `configure` sets a 16550's divisor of 1.8432 MHz within 3 % (`ns16550.rs`);
    - a PL011 and the channel keep 115200, the rate the firmware leaves, and refuse another (`invalid`).
- **`serial.wit` 1.1:** the UART chips named, nothing else changed (the bindings regenerated; `usb_serial` and `link` unchanged).
- **The build:** `uart_serial` in `02_build.sh`'s list, so x86 and aarch64 build it. It is not in `BOOT_FILES`, which with `init`'s start is 100000-KRN-0221's.
- **The test:** `tests/serial_host.rs` checks the 16550's divisor.
- **The docs:** [docs/link/mindlink.md](../docs/link/mindlink.md), "On a board's UART".

**What waits for 100000-KRN-0221:**
- plan step 3, `init`'s word and lending;
- step 6's QEMU runs: x86 with COM1 on a pty, `virt` with its PL011, then the channel on a tree whose console shares its page.

Until then `uart_serial` has not run on any machine or emulator (MC-12.3).

## Progress (2026-11-10)

**In `main` since round 106** (7415c9a1), through the kernel session's request:
- `uart_serial` (db50334a) and `link_update.py`'s race (b9fa8183);
- 100000-KRN-0221: `uart_serial` a boot image (handoff 30), started with `uart` in link.conf; `link` takes its client in place of `usb_serial`'s.

**Tested by the kernel session** (QEMU 8.2.2, TCG), in the `link` suite's new check: the console UART on a pseudo-terminal, `mindlink` put and got 16 KiB through it, on x86 (16550) and aarch64 `virt` (PL011). So plan step 3 and the first half of step 6 are done for those two emulators.

**Left:**
- plan step 6's `effector485` run in QEMU: `hello`, `log` and `shell` through `uart_serial`, and the boot's first lines as console text;
- the kernel's console channel (chip 5) on a machine whose console page holds other UARTs. No emulator has one. The first are the two Allwinner boards (main task [100004](100004-two-allwinner-boards-before-the-phone.md): the H5's UART0 page holds UART1 to UART3, the H618's UART1 to UART5), then the phone;
- U-Boot's prompt on the same line, through effector485: [800-LNK-0043](800-LNK-0043-effector485-a-line-without-link.md).

## Related

[100000-PRT-0010](100000-PRT-0010-a-16550-uart-in-memory.md), [100000-KRN-0171](../issues-done/100000-KRN-0171-platform-devices-from-the-device-tree.done), [800-LNK-0019](800-LNK-0019-the-link-as-the-owners-console.md), [800-LNK-0039](800-LNK-0039-effector485-as-the-effector-agents-engine.md), [docs/remote-serial.md](../docs/remote-serial.md).
