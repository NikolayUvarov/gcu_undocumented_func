# 230-KRN-0171 — Platform devices from the device tree

**Type:** kernel (ABI, `init`) · **Owner:** `KRN` · **Priority:** P2 · **Status:** open · **Blocked by:** [230-PRT-0009](230-PRT-0009-the-board-from-the-device-tree.md) · **Main task:** [230](230-a-phone-the-pinephone-first.md), stage 0 · **Roadmap:** track H · **Constitution:** MC-1.3, MC-1.5, MC-3.1, MC-12.4

## Problem

- **`PLATFORM_MMIO` and `PLATFORM_IRQ`** hand out only the board's UART and RTC, by a fixed index (`arch/aarch64/platform.rs`).
- **Every other driver finds its device on PCI.**
- **A phone has no PCI:** every device is a node of the device tree.

## Plan

- **The kernel lists the tree's device nodes** with `status = "okay"`: each node's compatible strings, its `reg` ranges, and its interrupts translated to GIC lines.
- **A new ABI version:**
  - `PLATFORM_MMIO` and `PLATFORM_IRQ` take a node's index;
  - a call lists the nodes (the index and the first compatible string), for `init` only, as the ACPI capabilities are.
- **`init`'s side:**
  - it names drivers by compatible strings in its table;
  - it grants each driver its node's MMIO and lines;
  - a range that overlaps RAM, or another node's range, is refused.
- **Unchanged:** DMA regions, as today.
- **Not handed out by the kernel:** a node's clocks, resets and regulators. They belong to the drivers of the clock controller and the PMIC (230, stage 2), reached by IPC.
- **The explicit transition:** the fixed indices go, with no compatibility code (AGENTS.md, section 3).

## Acceptance criteria

- **QEMU `virt,acpi=off`:**
  - the PL011 and the PL031 reach the shell and `rtc` through their nodes;
  - a node with `status = "disabled"` is not handed out.
- **Host tests** of the list's bounds.
- **The ABI's version** and docs/api updated.

## Related

[230](230-a-phone-the-pinephone-first.md), [230-PRT-0009](230-PRT-0009-the-board-from-the-device-tree.md), [205](205-aarch64-boards.md).
