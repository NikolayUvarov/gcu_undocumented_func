# 004 — IDT + Local APIC / IO-APIC instead of the 8259 PIC, interrupt-driven timer and keyboard

**Type:** feature (roadmap #2) · **Priority:** high · **Status:** open · **Blocked by:** [003](003-bss-and-heap-allocator.md) (statics are needed for IDT/GDT/TSS)
**Affects:** `kernel/`

## What the handoff requires

Rolled-back attempt: 8259 remap, IRQ0 (PIT) + IRQ1 (PS/2), asm context switching → Triple Fault after `sti` in the UEFI environment. We need to move to the APIC so that interrupts do not conflict with the state left by the firmware.

## Current state

There is no IDT, no GDT and no handlers in the code. Meanwhile the README describes "Hardware interrupts (IDT) for timing and keyboard input" as implemented — see [010](010-docs-sync.md).

## Plan (order matters — each step is verified in QEMU separately)

1. **Own GDT+TSS** (the UEFI GDT cannot be considered stable after `exit_boot_services`), `lgdt`, reload `cs/ss`; an IST stack for #DF.
2. **IDT with all exceptions** (`extern "x86-interrupt"`, feature `abi_x86_interrupt`), a #DF/#GP/#PF handler that prints the vector number and RIP to the framebuffer (otherwise any fault is a Triple Fault with no diagnostics). This was probably the real cause of the reset in the previous iteration.
3. **Mask the 8259** (`0xFF` to ports `0x21`/`0xA1`) — don't remap, just disable it.
4. **Local APIC**: `IA32_APIC_BASE` MSR (`0x1B`), enable `xAPIC` (or x2APIC via MSR — simpler, no MMIO mapping), SVR (`0xF0`), LVT Timer (`0x320`) in periodic mode with a divider; calibrate against the PIT or the TSC frequency from CPUID (in QEMU `0x15`/`0x16` is usually available).
5. **IO-APIC** for the keyboard: address from the ACPI MADT (take `ACPI_2_0_TABLE_GUID` from the `SystemTable` before `exit_boot_services` and pass the RSDP in `BootInfo`); ISA IRQ1 → GSI1 (taking Interrupt Source Overrides into account), redirected to a vector, e.g. `0x21`.
6. `sti` only after all of the steps; EOI via `0xB0`.
7. Then — a context switch (asm, saving registers into the TCB) and round-robin of two tasks: "kernel draws a circle" / "app draws a square".

## Acceptance criteria

- The timer tick counter increases and is visible on screen; the rate is stable (no `nop` delays).
- Key presses arrive via interrupts; the `0x64/0x60` polling is removed.
- An exception (`ud2` in test code) shows diagnostics instead of rebooting QEMU.
- The documentation in README/handoff is updated.
