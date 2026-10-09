# 174-KRN-0037 — Every vector state component for programs: AVX-512 and AMX

**Type:** kernel · **Owner:** `KRN` · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [174](174-full-use-of-pc-hardware.md) · **Constitution:** MC-1.8, MC-5.1, MC-12.1

## Problem

A task's saved vector state is x87, SSE and AVX only. `XCR0 = 0b111` and a fixed 1 KiB area on the interrupt stack and in every task (`kernel/src/arch/x86_64/mmu.rs:131`, `context.rs:6`). On a CPU with AVX-512 (Intel Xeon; AMD Zen 4 and 5) or AMX (Xeon from Sapphire Rapids), a program that uses them faults (#UD), so the fastest vector and matrix units of the machine are unused.

## Plan

- **The state components.** The boot CPU sets XCR0 to x87, SSE and AVX plus, where CPUID leaf 0xD lists them:
  - AVX-512's three components (opmask, ZMM_Hi256, Hi16_ZMM);
  - AMX's two (XTILECFG, XTILEDATA).
  The other CPUs follow.
- **The area's size** comes from CPUID 0xD (EBX for the enabled components), not a constant.
  - The interrupt entry reserves that much on the stack.
  - A task's saved context is allocated with that size and charged as before.
  - The IST stacks grow by it.
- **Layout.** The saved state becomes a pointer to the registers, then the XSAVE area at offset 64, then the registers, on the stack and in a task alike.
- **What programs are told.** `BootInfo.cpu_features` gains `FEATURE_AVX`, `FEATURE_AVX512` and `FEATURE_AMX`. These bits are additive; programs that do not know them ignore them.
- **`STAT_CPUS`** reports the enabled components as now (`FPU=XSAVE+AVX`, `+AVX512`, `+AMX` in `top`).
- **Test kernel `xsave-pad-test`.** QEMU's TCG emulates neither AVX-512 nor AMX. This kernel pads the area to AMX's size (11 KiB), so the busy, smp and isolation suites check the larger frames: the stacks, every task's context and the offsets.

## Acceptance criteria

- QEMU: the suites pass with `-cpu max` (AVX) and without AVX (FXSAVE), and with the padded kernel.
- An AVX-512 machine (an AMD Zen 4 or 5, or an Intel Xeon) runs a program that keeps ZMM and opmask values across preemption and other CPUs' work. The profile records the machine.

## Related

Issue 153 (AVX state per task), [174](174-full-use-of-pc-hardware.md).
