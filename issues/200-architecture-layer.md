# 200 — Architecture layer in the kernel and libmind (x86-64 first)

**Type:** kernel · **Owner:** porting track (with the kernel owner) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** track H · **Constitution:** MC-12.1 (profile per platform)

## Problem

The kernel mixes platform code with the rest:
- interrupt descriptor table, APIC, 8259/PIT;
- x86 page tables;
- context-switch assembly, `int 0x80`;
- the SMP trampoline, ACPI reset, port I/O for PCI, CPUID/RDTSC.

libmind does the same with its system-call stub, RDRAND and RDTSC. Another architecture cannot be added without first drawing the line.

## Plan

- `kernel/src/arch/x86_64/` with a small interface the rest of the kernel uses, and the generic kernel calls only that:
  - entry and exceptions, interrupt controller and timer;
  - address-space operations (map, unmap, protect, switch, TLB shootdown);
  - context save/restore and user entry;
  - CPU start, reset and power;
  - the clock source, the entropy source, the platform bus (PCI configuration access).
- `libmind/src/arch/x86_64.rs`: the system-call instruction, the cycle counter, the entropy instruction.
- `common/abi.rs` keeps only what every architecture shares. Architecture-specific constants move next to their code.
- x86 legacy drivers (`LEGACY:`) stay x86-only and are built only there.
- No change in behavior: every QEMU suite and host test passes unchanged, and the boot log is the same.

## Acceptance criteria

- `cargo build` of the kernel and libmind only reaches x86 code through `arch::`. A grep for `asm!` outside `arch/` finds nothing.
- All CI jobs green.
- `docs/profile/tcb.md` names the architecture layer in the TCB.

## Related

[201](201-aarch64-boot.md), [ROADMAP](../ROADMAP.md) track H.
