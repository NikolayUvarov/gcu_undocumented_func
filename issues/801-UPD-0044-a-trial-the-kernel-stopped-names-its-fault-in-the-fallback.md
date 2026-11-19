# 801-UPD-0044 — A trial the kernel stopped names its fault in the fallback's reason

**Type:** `updater`, `init` (`idl/init.wit`), the kernel's fault record · **Owner:** `UPD` (the kernel session) · **Priority:** P1 · **Status:** open (2026-11-19) · **Blocked by:** [801-KRN-0261](801-KRN-0261-every-stop-of-a-trial-falls-back-and-is-recorded.md) (the record) · **Main task:** [801](801-the-bench-stays-manageable.md) (S2) · **Constitution:** MC-9.4, MC-9.9, MC-12.3, MC-12.4

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

- [ ] A trial the kernel stopped on a fault is reported to the server with the fault's line, in the answer and in each heartbeat until a trial is confirmed (QEMU, x86).
- [ ] A record of this slot's own run (a confirmed slot that panicked) is not reported as a fallback.
- [ ] `init.wit`'s new version and the record's layout are in `docs/api`.

## Related

[801](801-the-bench-stays-manageable.md) (S2), [801-KRN-0261](801-KRN-0261-every-stop-of-a-trial-falls-back-and-is-recorded.md), [801-UPD-0037](../issues-done/801-UPD-0037-a-refused-trial-spent-and-its-reason-kept.done), [801-UPD-0042](../issues-done/801-UPD-0042-a-refused-trials-reason-reaches-the-server-whole.done).
