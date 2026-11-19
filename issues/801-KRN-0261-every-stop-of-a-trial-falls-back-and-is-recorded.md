# 801-KRN-0261 — Every stop the kernel chooses falls back on a trial and is recorded for the next boot

**Type:** kernel (`kernel/src/main.rs`, `firmware.rs`, `scheduler.rs`, the x86 interrupt table), `init`'s safe start, ABI 35 · **Owner:** `KRN` · **Priority:** P1 · **Status:** in progress (2026-11-19) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S1, S5) · **Constitution:** MC-6.8, MC-9.3, MC-9.4, MC-9.9, MC-12.3, MC-12.4

## Problem

A release that breaks the kernel or `init` must roll its slot back (801 S1). Since [801-KRN-0258](../issues-done/801-KRN-0258-a-trial-that-hangs-before-init-needs-a-watchdog.done), a panic on a trial does: it restarts the machine, and the loader boots the fallback.

**The other stops the kernel chooses did not** (read in `main`, `e921073fcd`). Each called `cpu::halt_all()`, which stops the watchdog and every CPU, so a trial that ended in one stayed stopped until a person came:
- an exception in the kernel (`KERNEL EXCEPTION VECTOR=…`, `scheduler.rs`);
- `init`'s exit (`INIT EXITED: SYSTEM HALTED`), as when a release's `init` faults;
- an interrupt with no handler (x86's `unexpected`), which said nothing at all.

**Nothing of a fault reached the next boot.** The screen held the report, and only a person could read it. So 000-KRN-0217's safe start took a run that ended in a panic for a cut power, and its step 3 (a panic as evidence) was not done.

## Plan

1. **One path for every stop the kernel chooses** (`fatal_end`), after its report:
   - the other CPUs are stopped;
   - the report's line is kept in a firmware variable (`MindCoreFault`, the run mark's vendor GUID): the format's version, the slot, the kind and the line;
   - a trial not yet confirmed restarts into the fallback, its watchdogs left running in case the restart does not come;
   - any other run stops as before, its watchdog stopped first.

   The panic, the exception in the kernel, `init`'s exit and an interrupt with no handler all take it. The record is skipped when the firmware is in use, as the fault may be inside it or another CPU may have stopped there.
2. **The next boot reads the record and deletes it.**
   - It says so on COM1 and the screen, with the slot when it is the other one's:
     - `MIND CORE KERNEL: THE LAST RUN OF THIS SLOT ENDED IN THE KERNEL'S PANIC: …`;
     - `… IN AN EXCEPTION IN THE KERNEL: …`;
     - `… AS INIT EXITED: …`.
   - When the record is this slot's and a fault of the kernel's own, it sets ABI 35's `BOOT_LAST_RUN_FAULTED` in the tasks' `BootInfo`.
   - `init`'s exit is recorded but sets no flag: no service's start is suspected of it.
3. **`init` takes the flag as a freeze's evidence**, as it takes `BOOT_WATCHDOG_RESTARTED`. What the run of this slot was starting of the safe start's list is left out at once: `[INIT] SAFE START: THE LAST RUN DID NOT END AS ASKED, THE KERNEL STOPPED IT ON A FAULT; NOT STARTED: …`.
4. **The test kernel `panic-test`.**
   - It panics at the first log system call of a boot after no record, as before.
   - In the boot after that panic, it panics on the tick 20 s past the boot, while the list is starting.
   - After that it does not panic.

## Acceptance criteria

- [ ] In QEMU (x86), the `boot` suite's panic check:
  - the panic is recorded as the machine stops;
  - the next boot names it;
  - the panic of that boot, 20 s in, leaves out at once what was starting.
- [ ] The panicked trial (801-KRN-0258's check): the trial records its panic and restarts, and slot A's kernel names slot B's panic. `init` takes no evidence from it.
- [ ] `init`'s exit and the ABI check still halt as before outside a trial (the `services` suite, the ABI kernel's check).
- [ ] aarch64 builds, and its suites pass. It runs no panic kernel, so no aarch64 evidence of the record is claimed.
- [ ] docs/profile says which stops fall back and are recorded, and what is not shown.

**Not claimed:** a bench machine's run, and a trial ended by an exception in the kernel, `init`'s exit or an interrupt with no handler. Those take the panic's path, but no test makes one.

**Left to [801-UPD-0044](801-UPD-0044-a-trial-the-kernel-stopped-names-its-fault-in-the-fallback.md):** the record's line reaches neither the updater nor the server, so a fallback from a trial the kernel stopped is reported without its step (S2).

## Related

[801](801-the-bench-stays-manageable.md) (S1, S5), [000-KRN-0217](000-KRN-0217-a-safe-start-that-leaves-out-only-what-it-suspects.md) (its step 3), [801-KRN-0258](../issues-done/801-KRN-0258-a-trial-that-hangs-before-init-needs-a-watchdog.done), [211-KRN-0013](../issues-done/211-KRN-0013-fatal-messages-on-the-screen.done) (the panic's report on the screen).
