# 005 — System calls (`int 0x80`) between userspace and the kernel

**Type:** feature (roadmap #3) · **Priority:** medium · **Status:** open · **Blocked by:** [004](004-apic-idt-interrupts.md)
**Affects:** `kernel/`, `app/`, shared protocol crate

## What the handoff requires

After the IDT/APIC are stabilized, bring back software vector `0x80` for requests from the application: time, hardware events, memory allocation.

## Current state

The application receives `&BootInfo` and writes to the framebuffer directly; there is no "upward" interface at all; it cannot return to the kernel (`-> !`). The README claims `int 0x80` is implemented — see [010](010-docs-sync.md).

## Plan

1. A shared `no_std` crate `mind-abi` (path dependency for kernel and app): syscall numbers, `#[repr(C)]` structs, `BootInfo` (eliminates the triple duplication, see [knowledge/05](../knowledge/05-observations-and-risks.md)).
2. Vector `0x80` in the IDT with `DPL=3` (for now everything is in ring 0 — DPL=0, but plan for it).
3. Convention: `rax` — number, `rdi/rsi/rdx/r10/r8/r9` — arguments, `rax` — result (like Linux). First calls:
   - `SYS_TICKS` → number of timer ticks;
   - `SYS_KEY_POLL` → last scancode or 0;
   - `SYS_ALLOC(size, align)` / `SYS_FREE(ptr, size, align)` → from the kernel heap (issue 003);
   - `SYS_EXIT` → return control to the kernel (currently impossible).
4. In app: `unsafe fn syscall1..3` wrappers via `asm!("int 0x80")`, use `SYS_TICKS` instead of the `nop` loop.
5. Later: `syscall/sysret` instead of `int 0x80` (faster, standard for x86_64), userspace in ring 3 with separate pages.

## Acceptance criteria

- app animates based on `SYS_TICKS`; the speed does not depend on the CPU.
- app does `SYS_EXIT` on the Esc key, and the kernel returns to its own loop.
- No protocol structure is duplicated between crates.
