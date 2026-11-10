# 100004-PRT-0014 — A kernel entered at EL2 goes on at EL1

**Type:** porting (kernel, aarch64) · **Owner:** `PRT` (the kernel session) · **Priority:** P1 · **Status:** open (made and tested in QEMU, 2026-11-10; landing and the boards' runs left) · **Blocked by:** — · **Main task:** [100004](100004-two-allwinner-boards-before-the-phone.md); [100000](100000-a-phone-the-pinephone-first.md), stage 1 · **Roadmap:** track H · **Constitution:** MC-1.2, MC-1.3, MC-12.1

## Problem

**`MOB`'s report** (its message of 2026-11-10 to `KRN` and `PRT` on `internal-dev-messages`, `…T0700Z-MOB-entered-at-el2-…`):

- **Where the boards start the loader.** TF-A hands U-Boot over at EL2 on both Allwinner boards, and U-Boot (v2026.10, without `ARMV8_SWITCH_TO_EL1`) starts EFI applications there. So `BOOTAA64.EFI` and the kernel after it run at EL2.
- **The other firmware.** The PinePhone's Tow-Boot is likely the same. EDK2 starts at EL2 wherever the core has it.
- **What happens then.** Neither the loader nor the kernel read `CurrentEL` or set anything of EL2.
  - The kernel ran at EL2 on U-Boot's tables.
  - `init`'s first `svc` was taken at EL2 by U-Boot's vectors: ESR 0x56000000, `"Synchronous Abort" handler … Resetting CPU`.
- **Why CI never showed it.** QEMU's `virt` has no EL2 without `virtualization=on`, so every CI group entered at EL1.

## Plan

1. **Leave EL2 at entry, on every CPU,** before anything relies on EL1. This is in the kernel, since the secondary CPUs come up through PSCI's `CPU_ON` at the EL the firmware chooses.
2. **Name the EL entered at** in the board's line.
3. **CI at EL2:**
   - the U-Boot group with `virtualization=on`;
   - an AAVMF group with `virtualization=on`, with the GICv3 system registers.

## Acceptance criteria

- **In QEMU with `virtualization=on`,** under U-Boot (GICv2, device tree) and under AAVMF (GICv3, ACPI):
  - the kernel names EL2 in its board line;
  - `init` and the services run on every CPU;
  - the suites pass as they do at EL1.
- **The groups entered at EL1** pass unchanged and name EL1.
- **On the boards** (100004-MOB-0002, 0003): the kernel boots past `INIT STARTED` and names EL2.

## Progress (2026-11-10)

On the branch `claude/100004-PRT-0014-leave-el2`.

**`kernel/src/arch/aarch64/cpu.rs`.** All of the code below lies between `ap_boot` and `ap_boot_end`, which the boot CPU writes back from its caches for CPUs that start with theirs off.

- **`el2_for_el1`** sets EL2 for a kernel at EL1:
  - `HCR_EL2` = RW alone: EL1 in AArch64, nothing trapped, no stage 2 (`VTTBR_EL2` 0);
  - `CNTHCTL_EL2` gives EL1 the physical counter and timer; `CNTVOFF_EL2` 0;
  - `CPTR_EL2` traps no FP or SIMD; `HSTR_EL2` 0;
  - `VPIDR_EL2` and `VMPIDR_EL2` hold the CPU's own `MIDR` and `MPIDR`, which EL1 reads;
  - where the core has the GICv3 system registers, `ICC_SRE_EL2` gives them to EL1, and `ICH_HCR_EL2` 0.
- **`to_el1`**, the boot CPU's, called first in `_start` (`cpu::kernel_level`). At EL2 it:
  - gives EL1 a map until `mmu::init` builds the kernel's own: `EL1_BOOT_L0` and `EL1_BOOT_L1`, the first GiB as device memory and the next 511 as normal memory, in 1 GiB blocks, as the kernel's own map treats them;
  - sets the kernel's MAIR and TCR (`mmu.rs`), so that `mmu::init`'s writes change nothing in between;
  - sets `SCTLR_EL1` with the MMU and caches on;
  - continues at EL1h, with DAIF masked, at the return address on the same stack.

  At EL1 it returns at once.
- **`ap_boot`**: a secondary CPU started at EL2 calls `el2_for_el1`, then `eret`s to its entry with its stack at EL1.
- **The board's line,** from the device tree and from ACPI, ends `ENTERED AT EL1` or `ENTERED AT EL2` (`board::ENTRY_EL`).
- **The budget.** This is 87 lines of the kernel, most of them assembly. `kernel/size-budget.json` is raised to 7500 in a commit of its own (7482 after it).

**Tests:**
- **`tests/qemu_smoke.py`'s `normal` suite** checks the board's line: EL2 with `virtualization=on`, EL1 without.
- **The `devicetree` suite** expects PSCI by SMC with `virtualization=on`, as QEMU then gives it.
- **Two groups run at EL2,** with their names kept:
  - "aarch64: the board from a device tree, under U-Boot";
  - "aarch64: RAM, ACPI and PCI above 4 GiB" (GICv3, ACPI).
- **Where they ran:**
  - locally, under U-Boot at EL2, the system came up on four CPUs. That run failed only on the new line check's pattern (the kernel ends that line `\r\r\n`), since fixed;
  - the runs after the fix are below.

**Left open:**
- **RAM in the first GiB.** The map before `mmu::init` makes the first GiB device memory, so a board whose kernel lies there (the Raspberry Pi 4's RAM starts at 0) needs more than this. None of the boards in 100004 or 100000 is such a board.
- **A firmware that names PSCI by HVC while starting the kernel at EL2** would be answered by its own EL2 vectors. TF-A and QEMU name SMC then.
- **The boards' runs** (100004-MOB-0002, 0003): no real core has run this yet.

## Related

[100004](100004-two-allwinner-boards-before-the-phone.md), [100004-PRT-0013](100004-PRT-0013-the-allwinner-boards-trees.md), [100000-PRT-0009](../issues-done/100000-PRT-0009-the-board-from-the-device-tree.done), [205](205-aarch64-boards.md).
