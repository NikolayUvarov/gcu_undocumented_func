# 230-PRT-0010 — A 16550 UART in memory, as the kernel's console and the shell's

**Type:** porting (the console) · **Owner:** `PRT` (the kernel session) · **Priority:** P2 · **Status:** open · **Blocked by:** [230-PRT-0009](230-PRT-0009-the-board-from-the-device-tree.md) (the console from `stdout-path`) · **Main task:** [230](230-a-phone-the-pinephone-first.md), stage 0 · **Roadmap:** track H · **Constitution:** MC-1.3, MC-12.1

## Problem

- **The consoles today:** aarch64's is a PL011; x86's is a 16550 on I/O ports.
- **The A64's UART0** is a Synopsys DesignWare APB UART. It is compatible with the 16550, but sits in memory, with `reg-shift = <2>` and `reg-io-width = <4>`.

## Plan

- **A 16550 in memory** for the kernel's lines and for the shell's console (`mind::dev::Uart`):
  - its registers at the node's address, with the node's shift and width;
  - the line as the firmware set it (115200 8N1 on the PinePhone), unless the tree's `current-speed` says otherwise;
  - the DesignWare UART's busy-detect interrupt handled (by reading `USR`);
  - transmission polled for the kernel, interrupts for the shell.

## Acceptance criteria

- **Host tests** of the register access against a model: shifts 0 and 2, widths 1 and 4, busy-detect.
- **On a platform:**
  - QEMU's `virt` has only a PL011;
  - whether another of QEMU's aarch64 machines carries a 16550 in memory is to be checked (Aspeed's AST2700 is a candidate);
  - otherwise the first test on a platform is the PinePhone (230, stage 1).

## Related

[230](230-a-phone-the-pinephone-first.md), [230-PRT-0009](230-PRT-0009-the-board-from-the-device-tree.md).
