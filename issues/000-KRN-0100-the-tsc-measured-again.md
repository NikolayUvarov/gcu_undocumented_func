# 000-KRN-0100 — The TSC measured again: one stretched 50 ms window no longer sets the clock for a whole boot

**Type:** kernel (x86 clock), tests · **Owner:** kernel session · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Constitution:** MC-5.6, MC-12.1

## Problem

**What failed:** the gate's "x86: files and block writes" failed twice on i9, on the branch's tips e83aebe and 50f361b, at the same check.
- The store suite puts a block, waits 61 s and fills the medium. The block's 60 s lease should have ended by then, so the fill's collection takes it.
- Instead, a later explicit `blocks collect` found it: `COLLECTED 1 BLOCKS 0 NAMES 1 SECTORS`.
- The same suite passed on this session's machine and on i9 before.

**Why:**
- Both failing boots measured the TSC at **3929 MHz**. i9's Linux, under WSL2, detects 3600.011 MHz, and calls the TSC unstable because it runs on Hyper-V.
- The LAPIC timer, measured over the same 50 ms, came out right: 62529 counts a tick against QEMU's 62500.
- So for those 50 ms, the TSC that QEMU's TCG passes through from the host ran 9% fast. After that it went back to its rate, while the kernel kept the number for the whole boot.
- The guest's clock then ran about 8% slow, and the test's 61 s were about 56 s to the guest.
- **Measured on i9 (50f361b):** a boot that calibrated 3604 MHz counted 64.32 s where the host counted 64.70 s, and the suite passed.

Real machines can stretch the window too: an SMI, or a hypervisor that runs the system.

## Plan

1. **On the boot CPU's tick, every second,** count the ACPI PM timer's ticks since the first measure: it wraps every 4.7 s at 24 bits.
2. **At 1, 8 and 60 s,** measure the TSC's rate over all that time. Where it differs by more than 0.1%:
   - move the clock's base so that now stays now, under a sequence number that readers check;
   - keep `MIND CORE KERNEL: CLOCK: TSC … MHZ, MEASURED AGAIN OVER … MS OF THE ACPI PM TIMER (WAS … MHZ)` among the kernel's lines for the hardware report. It does not go to the serial line, which by then is the shell's console: a line there broke the suites' typed commands.
3. **A test kernel** (`tsc-skew-test`) starts 9% high. A new group, "x86: TSC measured again", runs the `clock` suite on it.

## Acceptance criteria

- With `tsc-skew-test`, `CLOCK` reports the rate measured again (about 9% below the first), and over 30 s the guest's clock keeps the host's within 3%.
- "x86: files and block writes" passes on i9 again.
- `docs/profile/clocks.md` states the second measure and what is still not claimed.

## Progress

**2026-10-13:**
- Done as planned, in `kernel/src/arch/x86_64/clock.rs` and `interrupts.rs`.
- **The `clock` suite** on this session's machine (QEMU 8.2, 4 CPUs, TCG): 2948 → 2700 MHz, and a ratio of 1.000 over 30 s.
- **Left:** the gate on i9 and PCU.

## Related

[docs/profile/clocks.md](../docs/profile/clocks.md), 211-PRT-0003 (the tick without the PIT), 000-KRN-0065 (sleeps on the monotonic clock), 303-STO-0001 (the block store's leases).
