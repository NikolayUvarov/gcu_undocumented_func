# 100004-MOB-0003 — The first boot on the Orange Pi Zero 3 (H618, 2 GiB)

**Type:** porting (a board) · **Owner:** `MOB` · **Priority:** P1 · **Status:** open · **Blocked by:** [100004-MOB-0001](100004-MOB-0001-the-boards-sd-card.md) (the card, with U-Boot built from source for this board); the kernel leaving EL2 (asked of `KRN`, 2026-11-10); the board wired to a host ([issues-human, section 14](../issues-human/README.md#14-the-two-allwinner-boards-on-the-bench)); the kernel session's answer (100004's message to `KRN`) · **Main task:** [100004](100004-two-allwinner-boards-before-the-phone.md) · **Constitution:** MC-1.3, MC-12.1, MC-12.3

## Problem

The H618 has the H6/H616 layout:
- the GIC at 0x03021000;
- UART0 at 0x05000000, with UART1 to UART5 in the same page;
- the AXP313a on R_I2C.

Its 2 GiB are the PinePhone's smaller memory. Its first boot is the same as the H5's (100004-MOB-0002), with a second layout and four times the memory.

## Plan

1. **On the host:** the board's tree (Linux's `sun50i-h618-orangepi-zero3.dts`) through `common/fdt.rs`'s host tests. This is `PRT`'s, asked in 100004's message.
2. **In QEMU:** `virt` under U-Boot's UEFI with `-m 2G`, highmem off, as the board's memory ends below 4 GiB.
3. **On the board:** as 100004-MOB-0002's step 3, and also:
   - the memory the kernel takes from the UEFI map, against the 2 GiB (U-Boot and TF-A keep some);
   - whether U-Boot's SPL leaves the AXP313a's DCDC outputs as the DRAM needs them. The system does not touch the PMIC until 100004-MOB-0007.

## Acceptance criteria

- **The board boots to the shell or `link`** with four CPUs and its memory, and reboots on command.
- **Its log and configuration** are kept in this issue's progress (MC-12.1).

## Progress (2026-11-10)

- **The same stop at EL2** as on the H5 (100004-MOB-0002's progress): TF-A for the H616 also hands over to U-Boot at EL2.
- **Without a GOP:** this board's U-Boot has no video, so the loader finds no GOP. In QEMU under U-Boot without a display device the system reached `MIND>`.
- **The card:** 100004-MOB-0001's `dist/boards/mind-orangepi-zero3.img`.

## Related

[100004-MOB-0002](100004-MOB-0002-the-first-boot-on-the-h5.md), [100004-MOB-0007](100004-MOB-0007-the-axp313a-pmic.md), [100004-MOB-0008](100004-MOB-0008-memory-and-ipc-on-the-boards.md).
