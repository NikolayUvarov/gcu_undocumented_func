# 100000-LNK-0038 — `link` over the board's console UART, so that `effector485` reaches the PinePhone

**Type:** service, tool · **Owner:** `LNK` · **Priority:** P1 · **Status:** open · **Blocked by:** `init`'s lending of the UART (KRN, asked 2026-11-09); on the A64, 100000-KRN-0171's answer for the UART's page · **Main task:** [100000](100000-a-phone-the-pinephone-first.md), [800](800-mind-link.md) · **Constitution:** MC-2.3, MC-6.1, MC-12.3, MC-12.4

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

## Related

[100000-PRT-0010](100000-PRT-0010-a-16550-uart-in-memory.md), [100000-KRN-0171](100000-KRN-0171-platform-devices-from-the-device-tree.md), [800-LNK-0019](800-LNK-0019-the-link-as-the-owners-console.md), [800-LNK-0039](800-LNK-0039-effector485-as-the-effector-agents-engine.md), [docs/remote-serial.md](../docs/remote-serial.md).
