# 801-UPD-0045 — A trial the platform's watchdog restarted names it in the fallback's reason

**Type:** `updater`, `init` (`idl/init.wit`, a new minor version) · **Owner:** `UPD` (the kernel session) · **Priority:** P1 · **Status:** open (2026-11-20) · **Blocked by:** [801-UPD-0044](801-UPD-0044-a-trial-the-kernel-stopped-names-its-fault-in-the-fallback.md) (the fallback's question to `init`) · **Main task:** [801](801-the-bench-stays-manageable.md) (S2) · **Constitution:** MC-9.4, MC-9.9, MC-12.3, MC-12.4

## Problem

S2 asks that a fallback be reported with the step that failed. After 801-UPD-0044, the slot a trial fell back to reports:
- a trial `init` refused (801-UPD-0037);
- one the kernel stopped on a fault or at its deadline (801-KRN-0265).

**A trial that froze so hard that only the platform's watchdog restarted it** is still reported with no step. On the DEXP, 801-KRN-0259's trial built to stop before `init` came back as `fell back: slot A runs version 4, not 5` and nothing more. The boot after it knew: the kernel sets ABI 34's `BOOT_WATCHDOG_RESTARTED`.

**The flag alone is not enough.** It says that the last run was restarted, not whose run it was. After a fallback, slot A runs on with the records still naming B's spent trial. A freeze of A's own run later would be reported as B's.

**`init` knows the slot.** The run's mark, which `init` reads at boot (000-KRN-0217), holds the slot of the run that did not end as asked.

## Plan

1. **`init.wit` 1.15, `last-run`:**
   - the slot of the last run that did not end as asked (from its mark), and whether the watchdog restarted it (`BOOT_WATCHDOG_RESTARTED`);
   - `not-found` when the last run ended as asked.

   A function added at the end, a minor version.
2. **The updater**, at a fallback with no fault record (`last-fault` `not-found`), asks `last-run`.
   - When the watchdog restarted a run of the other slot, it keeps `why THE PLATFORM'S WATCHDOG RESTARTED THE TRIAL: IT STOPPED ANSWERING` in `refused`, as 801-UPD-0044 does.
   - A run of this slot, or one the watchdog did not restart, changes nothing.
3. **QEMU:** the `boot` suite's hung trial (801-KRN-0258's, x86's TCO on q35). Slot A's updater must name the watchdog; this needs a check of slot A's state.

## Acceptance criteria

- [ ] A trial the watchdog restarted is reported with that step, in the update state's failure and in `refused` (QEMU, x86).
- [ ] A freeze of the fallback slot's own run, with the records still naming the other slot's trial, is not reported as that trial's.
- [ ] `init.wit` 1.15 in docs/idl.

## Related

[801](801-the-bench-stays-manageable.md) (S2), [801-UPD-0044](801-UPD-0044-a-trial-the-kernel-stopped-names-its-fault-in-the-fallback.md), [801-KRN-0258](../issues-done/801-KRN-0258-a-trial-that-hangs-before-init-needs-a-watchdog.done), [801-KRN-0259](../issues-done/801-KRN-0259-the-pchs-over-clocking-watchdog.done), [000-KRN-0217](000-KRN-0217-a-safe-start-that-leaves-out-only-what-it-suspects.md).
