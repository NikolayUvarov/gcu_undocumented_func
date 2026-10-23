# 262-KRN-0165 — The boot CPU keeps its tick while it zeroes a large block

**Type:** kernel (`scheduler.rs`, `arch/*/interrupts.rs`, `arch/*/context.rs`) · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [262](262-model-runtime-on-the-machines-full-hardware.md) · **Constitution:** MC-2.13, MC-5.6

Found while checking [262-KRN-0160](../issues-done/262-KRN-0160-large-private-memory-for-a-named-program.done)'s reserve criterion (2026-10-23).

## Problem

- **The zeroing.** Since 262-KRN-0160, `ALLOC` zeroes a block of more than 64 MiB after its CPU lets go of the scheduler's lock. The work runs at the end of the system call, with that CPU's interrupts off.
- **What the boot CPU's tick drives.** Its tick ends every sleep and every IPC deadline in the system (000-KRN-0083), counts the uptime and feeds the watchdog. While the boot CPU zeroes, nothing in the system wakes from a sleep, on any CPU.
- **How long that lasts.**
  - Under QEMU's emulation (TCG), a GiB takes about 5 s.
  - A machine is expected to take about 0.1 s a GiB (not measured). A single block of 48 GiB would then take about 5 s.
- **The tick count on x86.** The x86 tick counts one per interrupt, so every tick missed in that time is lost from the uptime. The aarch64 tick counts the ticks that passed.
- **The clock.** Its measure of the TSC lost the PM timer's wraps over such a gap. That part was fixed in `clock.rs` (`pm_elapsed`, landing round 22). The sleeps still wait.

## Plan

1. **Zero in pieces** of 16 MiB.
2. **Take the tick between pieces.** On the boot CPU, between two pieces, a tick that is due is taken as its interrupt would be:
   - x86: the LAPIC's IRR bit for the tick vector;
   - aarch64: the timer's `ISTATUS`.

   Interrupts are opened for one instruction, with a per-CPU mark that the CPU is zeroing.
3. **Bookkeeping only during the zeroing.** While that mark is set, the entry handles the tick but never switches task: `Event::Clock` and `Event::Tick` run `timers` and the uptime, then return to the same frame. Any other interrupt stays pending until the zeroing ends.
4. **The other CPUs are unchanged.** They take no tick of their own while they zero, and a task pinned to them waits, as now.

## Acceptance criteria

- [ ] With 1 CPU (`--cpus 1`), while a program's 3 GiB are zeroed:
  - the shell answers `uptime` within 1 s each time (QEMU, x86 and aarch64);
  - the uptime stays within 2% of the host's time over the whole check.
- [ ] Host tests: a zeroing in pieces leaves the whole block zero, whatever the piece size and the block's length.
- [ ] `docs/profile/clocks.md` no longer lists the pause as not done, or states what is left of it.

## Related

[262-KRN-0160](../issues-done/262-KRN-0160-large-private-memory-for-a-named-program.done), [000-KRN-0100](../issues-done/000-KRN-0100-the-tsc-measured-again.done), 000-KRN-0083 (the 1 ms tick), [262](262-model-runtime-on-the-machines-full-hardware.md).
