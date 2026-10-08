# 211-KRN-0013 — The kernel's boot line and fatal messages on the screen

**Type:** kernel · **Owner:** `KRN` · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-10.2, MC-12.1

## Problem

The kernel writes its boot line, `KERNEL PANIC` and `KERNEL EXCEPTION` only to COM1 (`kernel/src/main.rs`; `docs/profile/threat-model.md`). Most PCs have no serial port. A first boot that stops in the kernel — for example on x2APIC (211-PRT-0002) — shows a frozen screen and no reason. The bootloader already prints its own errors on the screen.

## Plan

- A minimal text writer on the GOP framebuffer in the kernel: the built-in font, no scrolling beyond the screen. It is used only:
  - for the boot line and the steps before `init`;
  - for a panic or a kernel exception, which overwrites the screen's top lines;
  - for `INIT EXITED: SYSTEM HALTED`.
- After `init` starts, the screen belongs to the compositor as today. The kernel writes to it again only on a fatal stop.
- A test: the boot suite's panic kernel (`--panic-kernel`) shows the panic text on the screen (`screen_text`).

## Acceptance criteria

A kernel panic, in QEMU without a serial console attached, leaves its message readable on the screen. The boot line appears there before `init`. Nothing changes after `init` starts.

## Related

[211](211-intel-pc-from-a-sata-ssd.md), [211-PRT-0002](211-PRT-0002-x2apic.md).
