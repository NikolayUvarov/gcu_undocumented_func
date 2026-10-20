# 000-KRN-0136 — The update suite waits for the trial's confirmation, not for a prompt that line can split

**Type:** tests (`tests/qemu_smoke.py`, `update_suite`) · **Owner:** kernel session (the suite is the update track's; told on `internal-dev-messages`) · **Priority:** P2 · **Status:** made, in `fast-test` (2026-10-20); the gate is left · **Blocked by:** — · **Main task:** [351](351-self-update.md) · **Constitution:** MC-12.2

## Problem

Gate g0134 on un-rizen, "x86: files and block writes": the `update` suite failed after 1935 s. Version 6 had been fetched, slot B booted on trial and `init` had confirmed it, but the suite waited 1500 s for `MIND> ` in vain.
- The kernel's line `MIND CORE KERNEL: THE TRIAL BOOT IS CONFIRMED` came out inside the shell's first prompt: `M`, the line, then `IND>`.
- The suite already reads the updater's lines again when a kernel line lands in a command's echo (`logged`), but this wait for the first prompt after the restart did not allow for it.
- The same tree passed the suite on the kernel session's machine. When the line comes depends on the machine's speed.

## Plan

After `BOOT: SLOT B LOADED ON TRIAL` the suite:
- waits for the kernel's line whole, since the kernel prints it in one piece;
- then sends a newline;
- then waits for a fresh `MIND> `.

What it checks after that is unchanged.

## Acceptance criteria

- [ ] The `update` suite passes in a gate on x86 and aarch64 with this change.

## Related

[351-UPD-0021](../issues-done/351-UPD-0021-the-trial-confirmed-once-init-says-so.done) (the updater confirms when `init` says so), [000-KRN-0102](../issues-done/000-KRN-0102-init-says-how-the-boot-went.done).
