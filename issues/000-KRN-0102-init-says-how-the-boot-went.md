# 000-KRN-0102 — `init` says how the boot went

**Type:** `init`, `init.wit` 1.4 · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress (made; the `bench` suite passes on x86; the gate left) · **Blocked by:** — · **Main task:** — · **Constitution:** MC-9.5, MC-12.3

## Problem

The update track (requests-KRN.md, 2026-10-10, for [351-UPD-0007](../issues-done/351-UPD-0007-updater-service.done)): `init` confirms a healthy trial boot to the kernel (`BOOT_CONFIRM`, 351-KRN-0014), and the confirmed boot record is the updater's to write. The updater cannot ask whether the boot was confirmed, so it waits until 5 s past the kernel's deadline (120 s). The other slot stays untouched for two minutes after every update, and the QEMU `update` check waits that long.

## Plan

- **`init.wit` 1.4 `boot: func() -> result<boot, error>`:** the slot the machine booted from (`BOOT_SLOT_*`), whether it was on trial, and whether `init` confirmed it.
- **When it answers:** `init` serves its requests only after it has confirmed or refused the boot, so the answer is never "not yet": a trial boot answered `confirmed: false` is one `init` refused, and the kernel restarts the machine at its deadline.
- **Who may ask:** any client of `init`. The answer is what the bootloader handed the kernel and what `init` logged; it changes nothing.
- **The updater's use** is the update track's part: a note in [requests-UPD.md](requests-UPD.md).

## Acceptance criteria

- [ ] **`check`'s services group (`bench` suite, x86 and aarch64):** `the boot` matches the `BootInfo` the program was given, on the suites' boot from the volume's root (not on trial).
- [ ] **A trial boot:** the update track's `update` check writes the confirmed record within seconds of `[INIT] TRIAL BOOT CONFIRMED` once the updater asks `boot` (351-UPD-0007's criterion). Until then the trial path is not tested here.

## Related

[requests-UPD.md](requests-UPD.md) (the note), 351-KRN-0014 (the trial boot and `BOOT_CONFIRM`), [docs/profile/README.md](../docs/profile/README.md) (MC-9.5).
