# 262-KRN-0165 — The boot CPU keeps its tick while it zeroes a large block

**Type:** kernel (`scheduler.rs`, `arch/*/interrupts.rs`, `arch/*/context.rs`) · **Owner:** kernel session · **Priority:** P2 · **Status:** open (made on `claude/youthful-mendel-mf1soy`; landing left) · **Blocked by:** — · **Main task:** [262](262-model-runtime-on-the-machines-full-hardware.md) · **Constitution:** MC-2.13, MC-5.6

Found while checking [262-KRN-0160](262-KRN-0160-large-private-memory-for-a-named-program.md)'s reserve criterion (2026-10-23).

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
3. **Bookkeeping only during the zeroing.** While that mark is set, the entry handles the tick but never switches task: `Event::Clock` and `Event::Tick` run `timers` and the uptime, then return to the same frame. A device line is raised for its driver; a wake (a TLB flush, a task made ready here) is sent to the CPU again once the zeroing ends.
4. **The other CPUs are unchanged.** They take no tick of their own while they zero, and a task pinned to them waits, as now.

## Acceptance criteria

- [x] With 4 CPUs, while CPU 0 zeroes a program's 3 GiB, a 10 ms sleep of a program on CPU 1 ends within 1 s (QEMU, x86 and aarch64).
- [x] Host tests: the pieces cover a block once, none larger than 2 MiB, whatever the block's length.
- [x] `docs/profile/clocks.md` no longer lists the pause as not done, or states what is left of it.

**The first criterion as written on 2026-10-23 was wrong, and was replaced the same day before any of it was claimed.** It asked for the shell to answer within 1 s with 1 CPU. With one CPU, the shell shares the CPU that zeroes, and the kernel does not leave a system call for another task, so the shell waits whatever the tick does. What this task changes is the other CPUs' sleeps and deadlines, which the criterion now measures. The shell's own wait is stated in `docs/profile/clocks.md` as what is left.

## Progress (2026-10-23)

Made on `claude/youthful-mendel-mf1soy` (f918d2d6, 61105fbd, 053262be):
- **The kernel.**
  - `scheduler.rs`: the zeroing loop over `user_heap::pieces` (2 MiB), the per-CPU marks `ZEROING` and `WOKEN_ZEROING`, and `between_pieces`, the bookkeeping-only path of `interrupt`.
  - `arch/*/interrupts.rs`: `window()`, which opens the interrupts for one instruction (x86 `sti; nop; cli`; aarch64 `daifclr`, `isb`, `daifset`).
- **The test.**
  - `memtest naps <ms>` sleeps 10 ms at a time and prints its longest sleep.
  - In `big_memory_check`, a sleeper is put on CPU 1 and the 3 GiB on CPU 0, and `stat` checks where each landed. The longest sleep must stay under 1 s.
- **Checked** (this session's container, QEMU TCG, 4 CPUs, 6 GiB):
  - **The longest 10 ms sleep on CPU 1:**
    - x86, before (e8004e1c): 4461 ms, and the check failed;
    - x86, after: 26 ms;
    - aarch64 (`virt`, GICv3), after: 31 ms.
  - **Unchanged:**
    - `bigmem` (with the reserve) and `fill` on x86;
    - `smp` on 4 CPUs and on 1 CPU, and `heap` on 1 CPU, on x86;
    - aarch64 `smp` on GICv3 and on GICv2;
    - host test `the_pieces_cover_a_block_once_and_none_is_larger_than_2_mib`.
- **What is left.** The tasks of the CPU that zeroes still wait for the block: in the same runs the shell answered `uptime` in up to 12.8 s (x86) and 11.5 s (aarch64), because starting `uptime` goes through services on CPU 0. That needs a system call the kernel can leave, which is not planned. These are QEMU's emulation figures; a machine's are not measured (MC-12.1).

## Related

[262-KRN-0160](262-KRN-0160-large-private-memory-for-a-named-program.md), [000-KRN-0100](../issues-done/000-KRN-0100-the-tsc-measured-again.done), 000-KRN-0083 (the 1 ms tick), [262](262-model-runtime-on-the-machines-full-hardware.md).
