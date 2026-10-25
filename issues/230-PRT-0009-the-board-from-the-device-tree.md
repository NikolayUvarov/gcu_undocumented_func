# 230-PRT-0009 — The board from the device tree: the reader, and what every board has

**Type:** porting (kernel) · **Owner:** `PRT` (the kernel session) · **Priority:** P2 · **Status:** open · **Blocked by:** — ([210-KRN-0029](../issues-done/210-KRN-0029-device-tree-in-bootinfo.done): the tree's address in `BootInfo.device_tree`, done) · **Main task:** [230](230-a-phone-the-pinephone-first.md), stage 0 · **Roadmap:** track H · **Constitution:** MC-1.3, MC-12.1

## Problem

- **The aarch64 kernel reads the board from ACPI only** (`arch/aarch64/acpi.rs` into `board.rs`):
  - the GIC from the MADT;
  - the console from the SPCR;
  - the timer's interrupt from the GTDT;
  - the ECAM from the MCFG;
  - PSCI's conduit from the FADT.
- **With only a device tree,** as on the PinePhone under Tow-Boot, the kernel checks the tree's header and then boots with `virt`'s defaults.
- **[210-APL-0002](210-APL-0002-board-from-the-device-tree.md) plans the same reader for Apple Silicon,** with Apple's nodes. This task does the generic part; Apple's nodes (the AIC, the spin table, the DARTs, the Samsung-style UART) stay there.

## Plan

- **A bounds-checked FDT reader,** `kernel/src/arch/aarch64/fdt.rs`, as `acpi.rs` is for ACPI:
  - the header (version 17), the structure block and the strings;
  - `#address-cells` and `#size-cells`, `reg`;
  - `interrupts`, with `interrupt-parent` and `#interrupt-cells`;
  - `status`.
  - A malformed blob is refused without a panic.
- **From the tree into `board.rs`'s one record,** whichever source filled it:
  - the GIC: `arm,gic-400`, `arm,cortex-a15-gic`, `arm,gic-v3`;
  - the timer's interrupts: `arm,armv8-timer`;
  - the CPUs: `reg` as the MPIDR, `enable-method = "psci"`;
  - PSCI's conduit: `/psci`, `method`;
  - the console from `/chosen` `stdout-path`: `arm,pl011`, `snps,dw-apb-uart`, `ns16550a`;
  - `/reserved-memory` and the memory reservation block, kept out of the frames the kernel hands out;
  - a `simple-framebuffer` node, if the firmware gave no GOP.
- **Memory** still comes from the UEFI map.
- **The kernel's line:** `MIND CORE KERNEL: BOARD FROM THE DEVICE TREE …`.

## Acceptance criteria

- **Host tests** with:
  - QEMU `virt`'s tree, dumped with `-machine dumpdtb`;
  - the PinePhone's tree (Linux's `sun50i-a64-pinephone-1.2.dts`), its licence checked and recorded in THIRD_PARTY.md;
  - truncated and random blobs.
- **The `devicetree` suite** (QEMU `virt,acpi=off`) boots to the shell with the GIC, four CPUs through PSCI and the PL011, all read from the tree.
- **docs/profile/aarch64** says so.

## Related

[230](230-a-phone-the-pinephone-first.md), [210-APL-0002](210-APL-0002-board-from-the-device-tree.md), [230-KRN-0171](230-KRN-0171-platform-devices-from-the-device-tree.md), [230-PRT-0010](230-PRT-0010-a-16550-uart-in-memory.md), [205](205-aarch64-boards.md) (the board from ACPI).
