# 000-KRN-0039 — The kernel kept out of programs' pages: SMEP, SMAP and UMIP on x86, PAN on aarch64

**Type:** kernel · **Owner:** `KRN` · **Priority:** P1 · **Status:** in progress (done in QEMU; real machines' report lines left) · **Blocked by:** — · **Main task:** none · **Roadmap:** track K (kernel hardening) · **Constitution:** MC-1.2, MC-1.5, MC-12.1

## Problem

[knowledge/06](../knowledge/06-apple-security-lessons.md) lists this as a cheap P1 item, from Apple's PAN and PXN:

- **x86.** CR4 has neither SMEP nor SMAP nor UMIP set (`kernel/src/arch/x86_64/mmu.rs`). Two consequences:
  - A kernel bug that jumps to, or dereferences, a program's address runs the program's code in ring 0, or reads and writes its memory, with nothing to stop it.
  - Programs can read the GDT's and IDT's addresses (`sgdt`, `sidt`).
- **aarch64.** Programs' pages are PXN already, but PAN is not set.

The kernel reaches program memory through physical frames (`paging.rs` `readable`/`writable`), never through program addresses. Turning these on should therefore cost nothing.

## Plan

- **x86:**
  - The boot CPU sets SMEP (CR4.20), SMAP (CR4.21) and UMIP (CR4.11) where CPUID leaf 7 lists them, and the other CPUs follow.
  - The boot CPU sets them only after it switches to the kernel's page tables, because the firmware's tables may mark their pages as user pages.
  - The bits are cleared first, so nothing is inherited from the firmware.
- **aarch64:** where `ID_AA64MMFR1_EL1.PAN` is set, each CPU clears SCTLR_EL1.SPAN, so every exception entry sets PAN, and sets PSTATE.PAN.
- **Boot line and report:** `MIND CORE KERNEL: PROTECTION: SMEP SMAP UMIP` (x86; `NONE` without them) or `PXN PAN` (aarch64). The hardware report's kernel section says the same.
- **Tests:**
  - **The `isolation` suite** checks the boot line against the CPU model.
  - **New case `U`:** a program's `sgdt` faults (#GP) where UMIP is on and succeeds where it is not.
  - **Test kernel `protection-test` (x86):** on the first LOG call it reads the caller's mailbox page through the program's address, and jumps into its exit stub. Both must fault and resume through the fault fixup. It runs in CI group "x86: protection probes".
  - **aarch64:** the `smp` suite checks `PXN PAN` with `-cpu max`.

## Acceptance criteria

- **QEMU x86:**
  - With `-cpu max`, the line says `SMEP SMAP UMIP`, `sgdt` faults, and the probe kernel prints `READ OF A PROGRAM'S PAGE FAULTED, FETCH FROM IT FAULTED`.
  - With the default model (`qemu64`), the line says `NONE` and `sgdt` succeeds.
  - Every suite passes as before, since the kernel never touched program addresses.
- **QEMU aarch64 (`-cpu max`):** the line says `PXN PAN`, and every suite passes.
- **Real machines:** the hardware report of the next boot on the MacBook Pro and a PC states what was set.

## Progress

**2026-10-09: done in QEMU.**

- **x86** (`kernel/src/arch/x86_64/mmu.rs`):
  - `protection_offered` reads CPUID leaf 7.
  - `enable_protection` clears the bits. On the other CPUs it sets them; the boot CPU sets them in `protect_from_programs`, after `activate(kernel_root())`.
  - The boot line and the report's kernel section give `protection_names`.
- **aarch64** (`kernel/src/arch/aarch64/cpu.rs` `load`): SPAN is cleared and PAN set on each CPU where `ID_AA64MMFR1_EL1.PAN` is non-zero. The line is `PROTECTION: PXN PAN`.
- **Test kernel `protection-test`** (`kernel/src/arch/x86_64/report.rs` `probe`): the read and the jump resume through the existing kernel fault fixup.
- **QEMU runs passed:**
  - **x86 with `-cpu max`:** `SMEP SMAP UMIP`, `sgdt` faults, and the probe prints both faults. The isolation, smp, normal, services and net suites pass.
  - **x86 with `qemu64`:** `NONE`, and `sgdt` is allowed.
  - **aarch64 with `-cpu max`:** `PXN PAN`. The normal, smp and busy suites and `aarch64_smoke.py` pass.

**Left:** the line in the hardware report of a run on the MacBook Pro and on a PC (174-KRN-0038 writes it).

## Related

[knowledge/06](../knowledge/06-apple-security-lessons.md) (SMEP/SMAP row), [174-KRN-0038](174-KRN-0038-hardware-report.md) (the report), `docs/profile/threat-model.md`.
