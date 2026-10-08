# 210-APL-0006 — The console on the Samsung-style UART

**Type:** porting (kernel, `libmind`) · **Owner:** `APL` track (open) · **Priority:** P3 · **Status:** open · **Blocked by:** [210-APL-0002](210-APL-0002-board-from-the-device-tree.md), [210-APL-0003](210-APL-0003-aic-and-the-timer-fiq.md) (its interrupt); a Mac with M1 and a way to reach its UART ([issues-human](../issues-human/README.md#4-a-mac-with-apple-silicon-to-test-on)) · **Roadmap:** track H · **Constitution:** MC-12.1

Part of main task [210](210-apple-silicon-native.md), plan step 3.

## Problem

The kernel prints on a PL011 (`arch/aarch64/serial.rs`), and the shell's console uses `mind::dev::Uart`, which drives a 16550 or a PL011. The UART of an Apple Silicon Mac is Samsung-style (`apple,s5l-uart`: the ULCON, UCON, UFCON, UTRSTAT, UTXH and URXH registers), at the address the device tree gives. It is reachable only through a USB-C port switched to a debug mode by a USB Power Delivery vendor message: from a second Apple Silicon Mac with Asahi's `macvdmtool`, or with a serial adapter made for it (per Asahi's documentation).

## Plan

- The kernel's console on this UART when `/chosen` `stdout-path` names it (`serial.rs`, `PRT`'s directory: done with `PRT`).
- `mind::dev::Uart` learns the Samsung-style kind, chosen by the slot's capability as the PL011 is. The core of `libmind` is `KRN`'s: a request to `KRN`, or done with it.
- `PLATFORM_UART` hands out its registers and line from the device tree.

## Acceptance criteria

On an M1 with the debug connection, the kernel's lines and the shell's console reach the other machine's terminal, and input from it reaches the shell.

## Related

[210](210-apple-silicon-native.md), [205](205-aarch64-boards.md) (no console at a guessed address), [docs/profile/aarch64/](../docs/profile/aarch64/README.md).
