# 801-UPD-0046 — A trial the watchdog restarted with no mark of `init`'s is reported too, and a fallback is taken once

**Type:** `updater` · **Owner:** `UPD` (the kernel session) · **Priority:** P1 · **Status:** in progress (2026-11-20) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S2) · **Constitution:** MC-9.4, MC-9.9, MC-12.3

## Problem

S2 asks that after a fallback the slot that had the connection report the refused release's version, the step that failed and its lines. Since [801-UPD-0045](801-UPD-0045-a-trial-the-watchdog-reset-names-it-in-the-fallback.md), a trial the platform's watchdog restarted is reported as such. That holds only when `init`'s run mark names the trial's slot.

**The trial leaves no mark when:**
- it stops before `init` starts a service of the safe start's list, where `init` first writes its mark: in the kernel, or in `init` before it;
- it stops after `init` cleared its mark, a minute past `READY`. A trial waits for the server's word for up to 10 min (801 S1), so this is the common case of a trial that froze late.

Then the slot it fell back to logs `[UPDATER] SLOT B'S TRIAL FELL BACK TO THIS SLOT; THE KERNEL RECORDED NO FAULT OF ITS RUN, NOR THE WATCHDOG A RESTART`. That is wrong when the watchdog did restart it. `refused` is not written, and the server hears no step.

**Why the updater cannot take every such restart for the trial's.** The boot records keep naming the other slot's trial, fallen back, until the next update. So every later boot of the fallback slot asks again (801-UPD-0044's and 0045's checks show it).
- Without a mark, a later watchdog restart of this slot's own run looks the same as the trial's.
- Today only the slot named in a record or a mark keeps it from being taken for the trial's.

## Plan

1. **A fallback is taken once.** At the first boot of the fallback slot after a fallback, the updater keeps the newer record's sequence in `system/updater/fell-back`, after it asks how the trial ended.
   - At a later boot with the same newer record, it does not ask again. It says `[UPDATER] SLOT B'S TRIAL FELL BACK TO THIS SLOT AT AN EARLIER BOOT (RECORD n): WHAT ENDED IT WAS ASKED THEN`, and `refused`, if any, is reported as before.
   - A new trial writes newer records, so its fallback is taken afresh.
2. **No mark, and the watchdog restarted this boot**, at that first boot: the run the watchdog restarted was the trial's, the boot just before. The updater keeps `why THE PLATFORM'S WATCHDOG RESTARTED THE TRIAL: IT STOPPED ANSWERING`, as 801-UPD-0045 does.
3. **QEMU, the x86 `watchdog` suite:**
   - a trial of the `watchdog-test` kernel, which stops before `init`, is reset by q35's TCO;
   - slot A's updater keeps the watchdog's restart in `refused`;
   - at A's next boot it says the fallback was taken at an earlier boot, and keeps nothing new.

   801-UPD-0044's and 0045's checks of a later boot then expect that line.

## Acceptance criteria

- [ ] A trial the watchdog restarted, with no mark of `init`'s, is reported with that step, in the update state's failure and in `refused` (QEMU, x86).
- [ ] A later boot of the fallback slot, with the same newer record, does not ask how the trial ended and keeps nothing new (QEMU, x86).
- [ ] The groups that run 801-UPD-0044's and 0045's checks pass: `x86: boot, display, shell, memory, clock` and `x86: the watchdog`.

**Not covered:** the first boot of the fallback slot itself stopped by the watchdog before its updater kept the record's sequence. The boot after it would take that restart for the trial's.

## Related

[801](801-the-bench-stays-manageable.md) (S2), [801-UPD-0045](801-UPD-0045-a-trial-the-watchdog-reset-names-it-in-the-fallback.md), [801-UPD-0044](801-UPD-0044-a-trial-the-kernel-stopped-names-its-fault-in-the-fallback.md), [801-KRN-0258](../issues-done/801-KRN-0258-a-trial-that-hangs-before-init-needs-a-watchdog.done) (the `watchdog-test` kernel's stop before `init`).
