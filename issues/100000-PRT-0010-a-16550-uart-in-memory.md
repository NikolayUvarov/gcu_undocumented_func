# 100000-PRT-0010 — A 16550 UART in memory, as the kernel's console and the shell's

**Type:** porting (the console) · **Owner:** `PRT` (the kernel session) · **Priority:** P2 · **Status:** open (started 2026-11-08 at the maintainer's word, before the phone; the kernel's and the loader's lines done; the busy-detect and the host tests of the kernel's console done, 2026-11-10; below) · **Blocked by:** — ([100000-PRT-0009](../issues-done/100000-PRT-0009-the-board-from-the-device-tree.done), the console from `stdout-path`, done) · **Main task:** [100000](100000-a-phone-the-pinephone-first.md), stage 0 · **Roadmap:** track H · **Constitution:** MC-1.3, MC-12.1

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

**Found (2026-11-08): the console's page holds three other UARTs.**
- **The layout.** On the A64 each UART takes 0x400 bytes: UART0 at 0x01C28000, UART1 at 0x01C28400, UART2 at 0x01C28800, UART3 at 0x01C28C00, all in one 4 KiB page.
- **What the PinePhone's tree says of them** (Linux v6.12, `tests/data/fdt/pinephone-1.2.dtb`): UART1 carries the Bluetooth chip (`realtek,rtl8723cs-bt`), and UART3 is enabled as well.
- **Why it matters.** The kernel lends registers by whole pages, so lending UART0 to the shell's console (or to `link`) would lend the other three with it. That is more authority than the console needs (MC-2.3).
- **What follows:**
  - the shell's console on the A64 waits for a way that lends one UART alone. That is [100000-KRN-0171](../issues-done/100000-KRN-0171-platform-devices-from-the-device-tree.done)'s design question: the kernel as the console's only writer, or a narrower grant;
  - until then the kernel keeps UART0, as it does now (`platform::mmio` lends no 16550).

## Progress (2026-11-10): the busy-detect, and host tests of the kernel's console

`MOB` asked for it for the two Allwinner boards ([100004](100004-two-allwinner-boards-before-the-phone.md); its message of 2026-11-10 to `KRN` and `PRT`, point 2). Both boards' consoles are DesignWare UARTs ([100004-PRT-0013](100004-PRT-0013-the-allwinner-boards-trees.md)). On the branch `claude/100000-PRT-0010-busy-detect`:

- **Why the kernel needs it.**
  - **The interrupt.** A DesignWare UART raises its busy-detect interrupt (IIR 7) when LCR is written while it is busy. The interrupt stays up until `USR`, register 31 (0x7C at `reg-shift` 2), is read.
  - **Who writes LCR.** The kernel never writes LCR, but the firmware does when it sets the line up.
  - **When it matters.** The kernel takes the console's interrupt while it holds the console as a channel (100000-KRN-0222). A busy-detect left pending would then hold the line up, and each interrupt would find no byte to take.
- **The change** (`kernel/src/arch/aarch64/serial.rs`, `console_take`, one line of code).
  - Each time the console's interrupt is taken, the kernel reads IIR. If its low four bits are 7, it reads `USR`.
  - A plain 16550's IIR never reads 7, so the same code serves both. No flag for DesignWare is needed.
- **Host tests** (`tests/console16550_host.rs`, in `scripts/host_tests.sh`): a register file at shifts 0, 1 and 2 and widths 1, 2 and 4.
  - **What they check:**
    - the byte sent lands in register 0 at its width;
    - the room to send follows LSR's THRE;
    - bytes are taken while LSR's data ready is up;
    - the interrupts are set in IER;
    - no other register is written.
  - **How they see the `USR` read.** `USR` lies on a page that cannot be read, so a read of it ends a child process with SIGSEGV.
    - The tests show `USR` is read for IIR 0xC7 and 0x07.
    - They show it is not read for IIR's other answers (0xC1, 0xC4, 0xCC, 0xC2, 0xC6, 0x00, 0x01).
  - **Against the kernel without the line** the test fails at IIR 0xC7. With the line it passes.
- **What the tests do not show.** That reading `USR` clears the interrupt is from the DesignWare databook and Linux's `8250_dw.c` (`dw8250_handle_irq`). It has run on no UART yet: the boards' first boots (100004-MOB-0002, 0003) are the first.

**Not done:**
- the shell's console on a 16550 (`mind::dev::Uart`), with interrupts and the DesignWare busy-detect. `init` lends a console's node only when it is a PL011, so on these boards `link` and the shell reach the console through the kernel's channel (100000-KRN-0222);
- any run on a platform: QEMU's `virt` has only a PL011, and neither the boards nor the PinePhone have run it yet. The code above has run nowhere.

## Related

[100000](100000-a-phone-the-pinephone-first.md), [100000-PRT-0009](../issues-done/100000-PRT-0009-the-board-from-the-device-tree.done), [100000-KRN-0222](100000-KRN-0222-the-kernels-console-as-a-byte-channel.md) (the console as a channel), [100004](100004-two-allwinner-boards-before-the-phone.md) and [100004-PRT-0013](100004-PRT-0013-the-allwinner-boards-trees.md) (the two Allwinner boards, whose consoles are DesignWare UARTs).
