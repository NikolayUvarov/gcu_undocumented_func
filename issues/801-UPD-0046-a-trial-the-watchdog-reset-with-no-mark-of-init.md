# 801-UPD-0046 — A trial the watchdog restarted with no mark of `init`'s is reported too, and a fallback is taken once

**Type:** `updater` · **Owner:** `UPD` (the kernel session) · **Priority:** P1 · **Status:** in progress (2026-11-20) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S2) · **Constitution:** MC-9.4, MC-9.9, MC-12.3

## Problem

S2 asks that after a fallback the slot that had the connection report the refused release's version, the step that failed and its lines. Since [801-UPD-0045](../issues-done/801-UPD-0045-a-trial-the-watchdog-reset-names-it-in-the-fallback.done), a trial the platform's watchdog restarted is reported as such. That holds only when `init`'s run mark names the trial's slot.

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

- [x] A trial the watchdog restarted, with no mark of `init`'s, is reported with that step, in the update state's failure and in `refused` (QEMU, x86).
- [x] A later boot of the fallback slot, with the same newer record, does not ask how the trial ended and keeps nothing new (QEMU, x86).
- [x] The groups that run 801-UPD-0044's and 0045's checks pass: `x86: boot, display, shell, memory, clock` and `x86: the watchdog`.

**Not covered:** the first boot of the fallback slot itself stopped by the watchdog before its updater kept the record's sequence. The boot after it would take that restart for the trial's.

## Progress (2026-11-20)

**Made** on `claude/801-UPD-0046-a-trial-reset-with-no-mark`, on top of 801-UPD-0044's branch (round 159), as the plan says:
- **`updater`:** `system/updater/fell-back` holds `record n`, written once `init` answered how the trial ended. When `init` does not answer in 120 s, nothing is written, and the next boot asks again. A later boot with the same newer record says so and asks nothing.
  - At the first boot, `last-run`'s `not-found` with `BOOT_WATCHDOG_RESTARTED` set is taken for the trial's restart.
  - A record or mark of another slot is still no reason of the trial's.
- **The `watchdog` suite's `hung_trial_reported`**, with that suite's `watchdog-test` kernel in both slots.
- **0044's and 0045's checks of a later boot** expect the taken-once line.
- **Docs:** docs/update/updater.md 0.18 (EN and RU), docs/profile.

**Runs here** (QEMU, x86, 4 CPUs, this session's container), each PASS:
- `hung_trial_reported` in 51 s. Its lines:
  - `MIND CORE KERNEL: WATCHDOG TEST: THE TRIAL STOPPED FOR GOOD BEFORE INIT`;
  - `[UPDATER] SLOT B'S TRIAL OF VERSION not known FELL BACK TO THIS SLOT: THE PLATFORM'S WATCHDOG RESTARTED THE TRIAL: IT STOPPED ANSWERING; KEPT IN system/updater/refused`;
  - after a reset by hand, `[UPDATER] SLOT B'S TRIAL FELL BACK TO THIS SLOT AT AN EARLIER BOOT (RECORD 3): WHAT ENDED IT WAS ASKED THEN`.
- `own_freeze_after_a_fallback` in 127 s.
- `own_fault_after_a_fallback` in 56 s.

**On Effector's machines** (`f94d1f3fb4`, `main` `28b5b8dc22` merged), each PASS:

| Group | Machine | Time |
|---|---|---|
| x86: boot, display, shell, memory, clock | i9 | 1770 s |
| x86: the watchdog | x32 | 512 s |
| x86: a release that loses the server, rolled back | x32 | 727 s |
| x86: the updater and an update through the link | x32-2 | 1373 s |
| aarch64: the updater and the modes | acc-1 | 1169 s |
| aarch64: a release that loses the server, rolled back | acc-1 | 711 s |

The watchdog group's own lines were not kept, since it passed; the lines above are from the runs here.

## Related

[801](801-the-bench-stays-manageable.md) (S2), [801-UPD-0045](../issues-done/801-UPD-0045-a-trial-the-watchdog-reset-names-it-in-the-fallback.done), [801-UPD-0044](../issues-done/801-UPD-0044-a-trial-the-kernel-stopped-names-its-fault-in-the-fallback.done), [801-KRN-0258](../issues-done/801-KRN-0258-a-trial-that-hangs-before-init-needs-a-watchdog.done) (the `watchdog-test` kernel's stop before `init`).
