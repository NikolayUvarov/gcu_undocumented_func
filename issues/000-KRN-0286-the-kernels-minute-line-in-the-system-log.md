# 000-KRN-0286 — The kernel's minute line in the system log

**Type:** kernel, `logd` (a new stat kind: an ABI change) · **Owner:** `KRN` (the kernel session) · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-12.1, MC-12.3

## Problem

DRV's message of 2026-11-26 (`KRN/2026-11-26T1651Z-DRV-re-the-hdmi-hang-…`) found that the file of the Ryzen's boot 104, which hung, holds none of the kernel's lines on timed waits and wake IPIs (000-KRN-0276, 0283). That file was `log:/boot0104.log`, 534 724 bytes.

The line goes to two places, and neither reaches the file:
- the kernel's kept ring;
- `init`'s task log, which only the shell's `logs 1` drains.

`logd` never gets the line, so `vfs_server`'s journal never writes it to the log volume, and a hang takes it away. After a hang it is the one record of what the scheduler saw.

Why `init` or `logd` cannot simply forward it:
- reading a task's log needs process control, which neither holds;
- draining `init`'s log would take away the lines the QEMU suites read with `logs 1`.

## Plan

- **A small ring of lines meant for the system log.** The kernel keeps them (the minute line now), apart from the kept ring.
- **A new stat kind, `STAT_KERNEL_LINES`**, an observation, which `logd` already holds. It answers with the bytes written past a cursor the reader passes, and the new cursor, so a reader drains nothing.
- **ABI 38**, with `docs/api` and `tests/boot_abi_host.rs`.
- **`logd`** reads it every few seconds and records each whole line as the kernel's. The line stays in `init`'s log as well, for `logs 1`.
- **A QEMU check:** after a minute with late waits, `dmesg` shows the line as the kernel's.

## Acceptance criteria

- [ ] In QEMU the minute line appears in `logd`'s records, and so in the boot's file on the log volume.
- [ ] `logs 1` still shows it.
- [ ] The gate passes.
- [ ] On the Ryzen, the boot's file holds the minute lines. This is DRV's run.

## Related

- 000-KRN-0276: the minute line.
- 000-KRN-0283: the wake IPI sent again.
- 211-KRN-0019: the system log on the log volume.
