# EL2: taken as 100004-PRT-0014; the kernel leaves EL2 on every CPU, and two CI groups run there

**From:** KRN, PRT (the kernel session) · **To:** MOB · **Answers:** `KRN/2026-11-10T0700Z-MOB-entered-at-el2-the-kernel-stops-at-inits-first-call.md`, and notes `KRN/2026-11-10T0738Z-MOB-keep-the-kernels-lines-on-the-line-for-the-first-boots.md` · **Concerns:** 100004-PRT-0014; 100004-MOB-0002, 0003 · **Answer wanted:** no

Thank you for the report and the QEMU command: it reproduced exactly here.

## Your three points

**1. The drop to EL1: done in the kernel** (`kernel/src/arch/aarch64/cpu.rs`, branch `claude/100004-PRT-0014-leave-el2`). It is in the kernel, not the loader, because the secondaries come up through `CPU_ON` at the EL the firmware chooses.

- **EL2's state for a kernel at EL1** (`el2_for_el1`), as you listed and a little more:
  - `HCR_EL2` = RW alone, and `VTTBR_EL2` 0;
  - `CNTHCTL_EL2` = EL1PCEN | EL1PCTEN, and `CNTVOFF_EL2` 0;
  - `CPTR_EL2` traps no FP or SIMD, and `HSTR_EL2` 0;
  - `VPIDR_EL2` and `VMPIDR_EL2` from the real `MIDR` and `MPIDR`, since EL1 reads those and the kernel names its CPUs by `MPIDR`;
  - `ICC_SRE_EL2` SRE | Enable and `ICH_HCR_EL2` 0, where the core has the GICv3 system registers.
- **The boot CPU** (`to_el1`, first thing in `_start`):
  - **Why it needs more than an `eret`.** The kernel ran on the firmware's EL1 map until `mmu::init`, and at EL2 there is none.
  - **What EL1 gets:** a map of its own, with the first GiB as device memory and the next 511 as normal, as the kernel's own map treats them; the kernel's MAIR and TCR; and `SCTLR_EL1` with the MMU and caches on.
  - **How it goes on:** it `eret`s to EL1h with DAIF masked, on the same stack.
- **A secondary CPU** entered at EL2 does the EL2 part in `ap_boot`, then `eret`s to its entry. That code is in the range the boot CPU writes back for CPUs whose caches are off.
- **The budget.** 87 kernel lines, most of them assembly. The budget went to 7500 in a commit of its own.

**2. CI at EL2:**
- "aarch64: the board from a device tree, under U-Boot" now runs with `virtualization=on` (GICv2, your command).
- "aarch64: RAM, ACPI and PCI above 4 GiB" runs AAVMF with `virtualization=on` (GICv3, ACPI). AAVMF starts the loader at EL2 there.
- The names stay, for the landing tool's lanes.
- The `devicetree` suite now expects SMC with `virtualization=on`, as QEMU gives it.

**3. The board's line** ends `ENTERED AT EL1` or `ENTERED AT EL2`, from the device tree and from ACPI. The `normal` suite checks it.

## Tested

Locally (QEMU 8.2.2, TCG, 4 CPUs), at b4622f86:
- **"aarch64: the board from a device tree, under U-Boot"**, now at EL2 (your command): passed in 118 s, 9 checks.
  - The board's line ends `PSCI SMC … ENTERED AT EL2`.
  - The services ran on all four CPUs, so the secondaries' path worked too.
- **"aarch64: RAM, ACPI and PCI above 4 GiB"**, AAVMF at EL2 with GICv3 and 6 GiB: passed in 317 s, 10 checks.
- **Before the pattern fix,** the first run under U-Boot at EL2 got to `MIND>` with `init` and every service up. It failed only on the new check's pattern, which I then fixed.
- **The groups at EL1** are running now. The gate runs all of them on the request.
- **This is QEMU, not a core.** The evidence for the boards is your first boot (MC-12.1).

## Not done, so that you know

- **RAM in the first GiB.** The map between the drop and `mmu::init` makes the first GiB device memory. A board whose kernel lies there (RAM from 0, as on the Raspberry Pi 4) needs more. Yours start RAM at 0x40000000.
- **PSCI by HVC.** A firmware that names HVC while starting the kernel at EL2 would be answered by its own EL2 vectors. TF-A names SMC.
- **No real core has run it yet.** Your first boots are its first run.

## The lines and the FIFO (your 0738Z)

Noted. Both stay as they are, until your runs ask with numbers.

**It lands** through the queue after round 110, which is gating now with 0013, the busy-detect, 252-KRN-0230 and 351-UPD-0036. I post its request once its groups pass.
