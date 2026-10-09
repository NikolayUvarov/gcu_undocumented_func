# 174-KRN-0037 — Every vector state component for programs: AVX-512 and AMX

**Type:** kernel · **Owner:** `KRN` · **Priority:** P1 · **Status:** in progress (done in QEMU; an AVX-512 machine's run left) · **Blocked by:** — · **Main task:** [174](174-full-use-of-pc-hardware.md) · **Constitution:** MC-1.8, MC-5.1, MC-12.1

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

## Progress

**2026-10-09: every component enabled where the CPU lists it; checked in QEMU.**

- **XCR0.** The boot CPU sets x87, SSE and AVX. It adds AVX-512's three components and AMX's two where CPUID 0xD lists all of a group (`mmu::components`). The other CPUs set the same value.
- **The area.** Its size is CPUID 0xD's EBX for the enabled components, rounded up to 64 bytes (FXSAVE: 512 bytes). It replaces the fixed 1 KiB area (`context::set_area`, `area`, `size`).
  - **Layout:** the pointer to the registers at offset 0, the area at offset 64, then the registers, on the stack and in a task alike. The entry reserves `area` bytes on the stack.
  - **Allocation:** a task's saved context is allocated with `size()` and charged as before.
  - **IST stacks:** grow by one saved state.
- **Boot line:** `MIND CORE KERNEL: VECTOR STATE: XSAVE, XCR0 0x7, 832 BYTES A TASK` (FXSAVE and 512 without AVX). The hardware report's kernel section gives the same.
- **Programs:** `BootInfo.cpu_features` has `FEATURE_AVX`, `FEATURE_AVX512` and `FEATURE_AMX` (additive bits). `STAT_CPUS.xsave` carries the enabled components.
  - The shell's `cpus` names only `XSAVE+AVX`. Naming `+AVX512` and `+AMX` is requested from `APP` ([requests-APP.md](requests-APP.md)).
- **The busy fixture** keeps zmm0's upper half, zmm31 (Hi16_ZMM) and k1 across preemption where the kernel enables AVX-512, and says `AVX-512`. QEMU cannot run that path. An AVX-512 machine will.
- **Test kernel `xsave-pad-test`.** It pads the area to 11 KiB, AMX's size.
  - CI group "x86: padded vector area" runs it with `-cpu max` in the busy, smp and isolation suites.
  - The suites check the boot line: XSAVE with `-cpu max`, FXSAVE without, 11264 bytes with the padded kernel.
- **QEMU runs passed:**
  - busy, smp, isolation and normal without AVX (FXSAVE);
  - busy and smp with `-cpu max` (XSAVE, AVX), on 4 CPUs and on 1;
  - busy, smp and isolation with the padded kernel.

**Left:**
- A run on an AVX-512 machine (an AMD Zen 4 or 5, or an Intel Xeon), recorded in the profile.
- AMX on a Xeon from Sapphire Rapids.
- Each entry saves the whole area, and a switch copies it. XSAVEC or XSAVES, saving straight into the task's area, would cut that cost on CPUs with large areas. That is a later step of 174.

## Related

Issue 153 (AVX state per task), [174](174-full-use-of-pc-hardware.md).
