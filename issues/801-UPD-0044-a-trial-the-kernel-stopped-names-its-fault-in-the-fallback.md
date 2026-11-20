# 801-UPD-0044 — A trial the kernel stopped names its fault in the fallback's reason

**Type:** `updater`, `init` (`idl/init.wit`), the kernel's fault record · **Owner:** `UPD` (the kernel session) · **Priority:** P1 · **Status:** in progress (2026-11-20) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S2) · **Constitution:** MC-9.4, MC-9.9, MC-12.3, MC-12.4

## Problem

S2 asks that after a fallback the slot that had the connection report to the server:
- the refused release's version;
- the step that failed;
- the lines that show it.

**A trial `init` refused does that** (801-UPD-0037, 801-UPD-0042): the updater keeps `init`'s reason in `system/updater/refused`, and the agent reports it in each heartbeat until a trial is confirmed. On the DEXP, release 15 without `netstack.elf` came back as `NO HEARTBEAT ACCEPTED IN 600 S: THE AGENT SAID NOTHING` (801 S8, 2026-11-19).

**A trial the kernel stopped does not.** This covers:
- a panic, as in 801-KRN-0258's check;
- since 801-KRN-0261, an exception in the kernel, `init`'s exit and an interrupt with no handler.

The machine falls back, but nothing writes `refused`. The agent's answer is then only `fell back: slot A runs version 14, not 15`, with no step, and the heartbeat reports no fallback.

801-KRN-0261 keeps the fault's line in a firmware variable, and slot A's kernel names it on COM1 and the screen. The line reaches neither the updater nor the server.

## Plan

1. **The record, read by `init`.** `init` holds the firmware variables already (the run's mark).
   - The record's name and layout go to `common/abi.rs` beside the run mark's, with its format's version.
   - The kernel reads the record and leaves it; `init` reads it and then deletes it.
   - A record that `init` does not delete is named again at the next boot. That happens only when `init` did not run that far, and the boot after that one says so.
2. **`init` gives it to the updater** with the boot's state (`idl/init.wit`, a new minor version): the record's slot, kind and line, when the record is the other slot's.
3. **The updater keeps it as a trial's refusal.** A fallback from a trial whose kernel stopped is kept in `refused`, in the form `init`'s refusals have (the version, then `why THE KERNEL STOPPED THE TRIAL: …`). The agent then reports it as it reports those (801-EFF-0029), with no change on EFF's side.
4. **QEMU, the `rollback` suites:** a release whose kernel panics on its trial, sent through the agent, comes back `fell-back`. The server hears the panic's line in the heartbeat. x86 only; aarch64 runs no panic kernel.

## Acceptance criteria

- [x] A trial the kernel stopped on a fault is reported to the server with the fault's line, in the answer and in each heartbeat until a trial is confirmed (QEMU, x86).
- [x] A record of this slot's own run (a confirmed slot that panicked) is not reported as a fallback.
- [x] `init.wit`'s new version and the record's layout are in `docs/api`.

## Progress (2026-11-20)

**Made** on `claude/801-UPD-0044-kernel-fault-reported`, with 801-KRN-0265 (steps 1 and 2 of the plan are that task's).

- **When the updater asks.** It asks only at a boot of a slot, not on trial, whose newer boot record names the other slot's trial, unconfirmed, falling back to this slot: the trial ended there. It asks `init`'s `last-fault` before it serves any request, so that the agent's first view of the state already names the step; `init` answers once it started the boot services, and the updater gives up after 120 s.
- **What it keeps.** A record of the other slot's run is kept in `system/updater/refused`, after `init`'s refusal if one is there, as it is the last run's word:
  - `refused version V in slot B, falling back to slot A`;
  - `why THE KERNEL STOPPED THE TRIAL IN ITS PANIC: <the report's line>`, or `ON AN EXCEPTION IN THE KERNEL`, `AS INIT EXITED`, `ON AN INTERRUPT WITH NO HANDLER`, `AT ITS DEADLINE`;
  - `log not known`.

  The state's failure says the same at once.
- **What it leaves.** No record, or one of this slot's own run, leaves everything as before.
- **The check.** The x86 `rollback` suite gained release 7, which carries the `panic-test` kernel, sent through the agent with one try. Its trial panics at `init`'s first log line, and slot A comes back. The heartbeat must carry `version 7 in slot B, falling back to slot A: THE KERNEL STOPPED THE TRIAL IN ITS PANIC: panic test at src/scheduler.rs:…`; the answer must be `fell back: slot A runs version 5, not 7; the updater: …`; `refused` must hold the same.
- **Both slots must carry it.** A fallback slot whose release predates it deletes the record in its kernel and reports no step: the DEXP's slot A (release 14) is such a slot until a release with it is confirmed there.

## Tests (2026-11-20)

**The new check** passed in the x86 `rollback` suite on i9 (`c8a5d6c8ad`, 854 s), after two fixes to the check itself:
- `0a84d597b0`: slot B's trial and slot A's boot now run in one QEMU. Without flash, OVMF keeps a variable written after the boot services only in memory, which a restart keeps and a new QEMU does not.
- `c8a5d6c8ad`: the panic's report is read from the output, as the boot suite reads it, since `expect()` takes a panic for a failure.

Its lines:
- `[UPDATER] SLOT B'S TRIAL OF VERSION 7 FELL BACK TO THIS SLOT: THE KERNEL STOPPED THE TRIAL IN ITS PANIC: panic test at src/scheduler.rs:1496:141 CPU=0 PID=1 NAME=init; KEPT IN system/updater/refused`, 2.1 s into slot A's boot.
- `PASS: rollback: release 7, whose kernel panics on its trial, taken through the agent: …; the heartbeat carried the panic's line and the update was answered fell-back with it (801-UPD-0044)`.

**The check reads one heartbeat.** That every heartbeat carries the line until a trial is confirmed follows from `refused`, which each start of the updater reads again, as for `init`'s refusals (801-UPD-0042's runs).

**The groups it touches**, with 801-KRN-0265's (QEMU only, MC-12.1):
- `x86: the updater and an update through the link`: x32, PASS, 1377 s.
  - On i9 it failed once (`u44d`): the shell stopped answering `dmesg` 63 s into a boot of a confirmed slot, every CPU idle. That boot does not take the new path; on x32 it passed.
- `x86: services, storage, audio`: i9, PASS, 1202 s.
- The x86 boot group and the aarch64 groups are in 801-KRN-0265.

**Not shown by a run:** a record of this slot's own run alongside a fallback. The updater asks only when the records show the other slot's trial fell back, and keeps the record only when it is that slot's (`fell_back`); no test makes the two disagree.

## Landed (2026-11-20)

In round 157 (main `1025ac6bbd`), the gate green on both architectures; its x86 `rollback` group ran the new check (x32, 722 s).

**Left:** the second criterion has no run yet, and the slot the DEXP falls back to needs a release that carries this. 801-UPD-0045 takes the trial the watchdog restarts.

## The second criterion (2026-11-20)

**The check** (`own_fault_after_a_fallback`, the x86 `boot` suite, on `claude/801-UPD-0044-own-fault-not-a-fallback`). Both slots hold the `panic-test` kernel:
1. Slot B's trial panics at `init`'s first log line, and slot A boots with B's one try spent. A's updater keeps B's panic.
2. A's kernel panics 20 s in, on a run of its own. It is not a trial, so it stops, and the check restarts it.
3. At A's next boot the boot records still name B's trial fallen back to A, and the kernel's record is A's own.

The check asks:
- that the updater says `[UPDATER] SLOT B'S TRIAL FELL BACK TO THIS SLOT; THE KERNEL'S RECORD IS OF ANOTHER SLOT'S RUN: NOT ITS REASON`;
- that neither its log nor `update status` names A's panic;
- that `refused` still holds B's panic, kept at A's first boot.

**Runs:** three here, in QEMU (x86, 4 CPUs, this session's container), each PASS in 57 to 60 s. The lines of the first run:
- `MIND CORE KERNEL: THE LAST RUN OF THIS SLOT ENDED IN THE KERNEL'S PANIC: panic test, 20 s past the boot at src/arch/x86_64/interrupts.rs:80:120 CPU=0`;
- `[UPDATER] A TRIAL REFUSED BEFORE (system/updater/refused): refused version not known in slot B, falling back to slot A; THE KERNEL STOPPED THE TRIAL IN ITS PANIC: panic test at src/scheduler.rs:1496:141 CPU=0 PID=1 NAME=init`;
- `[UPDATER] SLOT B'S TRIAL FELL BACK TO THIS SLOT; THE KERNEL'S RECORD IS OF ANOTHER SLOT'S RUN: NOT ITS REASON`;
- `UPDATE: THE LAST FAILURE: version not known in slot B, falling back to slot A: THE KERNEL STOPPED THE TRIAL IN ITS PANIC: panic test at src/scheduler.rs:1496:141 …`.

With 801-UPD-0045 (round 158), the updater's line for this case became `[UPDATER] SLOT B'S TRIAL FELL BACK TO THIS SLOT; WHAT ENDED THE LAST RUN WAS ANOTHER SLOT'S: NOT ITS REASON`, and the check now asks for that line. It passed once more here on round 158's candidate merged with this branch (56 s).

**On Effector's machines** (`6fd343ffcf`, `main` merged): i9, `x86: boot, display, shell, memory, clock` PASS, 1791 s, with the check's line `PASS: a panic of slot A's own run, after slot B's trial fell back to it, is no reason of B's …`.

**Left:** the slot the DEXP falls back to needs a release that carries this.

## Related

[801](801-the-bench-stays-manageable.md) (S2), [801-KRN-0261](../issues-done/801-KRN-0261-every-stop-of-a-trial-falls-back-and-is-recorded.done), [801-UPD-0037](../issues-done/801-UPD-0037-a-refused-trial-spent-and-its-reason-kept.done), [801-UPD-0042](../issues-done/801-UPD-0042-a-refused-trials-reason-reaches-the-server-whole.done).
