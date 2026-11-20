# 801-KRN-0265 — init takes the kernel's fault record and answers it (`init.wit` 1.14)

**Type:** kernel (`kernel/src/firmware.rs`, `trial.rs`), `init`, `idl/init.wit`, `common/abi.rs` · **Owner:** `KRN` · **Priority:** P1 · **Status:** in progress (2026-11-20) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S2) · **Constitution:** MC-6.8, MC-9.4, MC-9.9, MC-12.3, MC-12.4

## Problem

Since [801-KRN-0261](../issues-done/801-KRN-0261-every-stop-of-a-trial-falls-back-and-is-recorded.done), the kernel keeps a record of each stop it chooses (a panic, an exception in the kernel, `init`'s exit, an interrupt with no handler) in the firmware variable `MindCoreFault`. The next boot's kernel names it on COM1 and the screen, then deletes it.

**No task can read it.** So the slot a trial fell back to cannot say why the trial ended. [801-UPD-0044](801-UPD-0044-a-trial-the-kernel-stopped-names-its-fault-in-the-fallback.md) needs the record in the updater of that slot.

**A trial the kernel restarts at its deadline leaves no record at all.** That is a trial `init` never confirmed: a boot service that did not start, the boot volume not mounted, or `init` stuck.

## Plan

1. **The record's layout goes to `common/abi.rs`:**
   - its name, `FAULT_RECORD_NAME`;
   - its format's version, `FAULT_RECORD_VERSION` (1);
   - the kinds `FAULT_*`;
   - the line's bound, `FAULT_RECORD_TEXT` (160).

   The kernel takes them from there.
2. **The kernel names the record and leaves it.**
3. **`init` reads the record as the boot starts, beside the run's mark, and deletes it.**
   - It says so in the system log: `[INIT] THE LAST RUN, OF SLOT B, ENDED IN THE KERNEL'S PANIC; THE KERNEL'S RECORD DELETED: …`.
   - It keeps the record for `last-fault`.
   - A record `init` could not delete is named again at the next boot, and the line says so.
4. **`init.wit` 1.14, `last-fault`:** the record this boot took (slot, kind, line), or `not-found`. Any client may ask. Adding a function at the end is a minor version (docs/idl, Evolution).
5. **A trial the kernel restarts at its deadline is recorded too,** as `FAULT_DEADLINE` (5) with the line `THE TRIAL BOOT WAS NOT CONFIRMED IN TIME`. It sets no `BOOT_LAST_RUN_FAULTED`: no service's start is suspected of it.

## Acceptance criteria

- [ ] The `boot` suite's panic check (x86) and the panicked trial still pass. The second boot's panic, 20 s in, comes after `init` took the record.
- [ ] In the x86 `rollback` suite, slot A's `init` names the trial's panic and deletes the record (801-UPD-0044's check).
- [ ] aarch64 builds, and its suites pass.
- [ ] docs/api describes the record; docs/idl lists 1.14; docs/profile says who deletes it.

**Not claimed:** a run of the deadline's record. No test restarts a trial at its deadline with the record checked.

## Related

[801](801-the-bench-stays-manageable.md) (S2), [801-KRN-0261](../issues-done/801-KRN-0261-every-stop-of-a-trial-falls-back-and-is-recorded.done), [801-UPD-0044](801-UPD-0044-a-trial-the-kernel-stopped-names-its-fault-in-the-fallback.md), [000-KRN-0217](000-KRN-0217-a-safe-start-that-leaves-out-only-what-it-suspects.md).
