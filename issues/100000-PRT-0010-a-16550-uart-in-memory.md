# 100000-PRT-0010 — A 16550 UART in memory, as the kernel's console and the shell's

**Type:** porting (the console) · **Owner:** `PRT` (the kernel session) · **Priority:** P2 · **Status:** open (started 2026-11-08 at the maintainer's word, before the phone; the kernel's and the loader's lines done, below) · **Blocked by:** — ([100000-PRT-0009](../issues-done/100000-PRT-0009-the-board-from-the-device-tree.done), the console from `stdout-path`, done) · **Main task:** [100000](100000-a-phone-the-pinephone-first.md), stage 0 · **Roadmap:** track H · **Constitution:** MC-1.3, MC-12.1

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
  - otherwise the first test on a platform is the PinePhone (100000, stage 1).

## Progress (2026-11-08)

**Done, with 100000-PRT-0009 (`claude/100000-PRT-0009-board-from-the-device-tree`):**
- **The kernel's lines through a 16550 in memory** (`kernel/src/arch/aarch64/serial.rs`):
  - register `n` at `n << reg-shift`, read and written `reg-io-width` bytes wide (1, 2 or 4), as the tree's `stdout-path` names them (`common/fdt.rs`, `UartKind::Ns16550`);
  - transmission polled on LSR's THRE, the wait bounded;
  - the line as the firmware left it; `current-speed` and `stdout-path`'s speed are read but not set.
- **The loader's own lines** the same way (`bootloader/src/main.rs`, `mod line`), from the SPCR or the tree, never from a fixed address.
- **A 16550 is not lent to the shell's console** (`platform::mmio`), which drives a PL011 only: it stays the kernel's.

**Not done:**
- the shell's console on a 16550 (`mind::dev::Uart`), with interrupts and the DesignWare busy-detect;
- the host tests against a register model (shifts 0 and 2, widths 1 and 4, busy-detect);
- any run on a platform: QEMU's `virt` has only a PL011, and the PinePhone is not at hand. The code above has run nowhere.

## Related

[100000](100000-a-phone-the-pinephone-first.md), [100000-PRT-0009](../issues-done/100000-PRT-0009-the-board-from-the-device-tree.done).
