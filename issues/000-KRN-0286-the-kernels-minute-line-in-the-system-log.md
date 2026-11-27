# 000-KRN-0286 — The kernel's minute line in the system log

**Type:** kernel, `logd` (a new stat kind: an ABI change) · **Owner:** `KRN` (the kernel session) · **Priority:** P1 · **Status:** in progress (in `main` since round 181, 10f8602b4c, 2026-11-27; DRV's run on the Ryzen left) · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-12.1, MC-12.3

## Problem

DRV's message of 2026-11-26 (`KRN/2026-11-26T1651Z-DRV-re-the-hdmi-hang-…`) found that the file of the Ryzen's boot 104, which hung, holds none of the kernel's lines on timed waits and wake IPIs (000-KRN-0276, 0283). That file was `log:/boot0104.log`, 534 724 bytes.

The line goes to two places, and neither reaches the file:
- the kernel's kept ring;
- `init`'s task log, which only the shell's `logs 1` drains.

`logd` never gets the line, so `vfs_server`'s journal never writes it to the log volume, and a hang takes it away. After a hang it is the one record of what the scheduler saw.

Why `init` or `logd` cannot simply forward it:
- reading a task's log needs process control, which neither holds;
- draining `init`'s log would take away the lines the QEMU suites read with `logs 1`.

## Plan (made)

- **A small ring of lines meant for the system log.** The kernel keeps them (the minute line now), apart from the kept ring.
- **A new stat kind, `STAT_KERNEL_LINES`**, an observation, which `logd` already holds. It answers with the bytes written past a cursor the reader passes, and the new cursor, so a reader drains nothing.
- **ABI 38**, with `docs/api` and `tests/boot_abi_host.rs`.
- **`logd`** reads it when a request comes, at most every 5 s, and at least once a minute when none does (a wake every few seconds made the one-CPU checks late on a slow machine), and records each whole line as the kernel's. The line stays in `init`'s log as well, for `logs 1`.
- **A QEMU check:** after a minute with late waits, `dmesg` shows the line as the kernel's.

## Acceptance criteria

- [x] In QEMU the minute line appears in `logd`'s records as the kernel's: the services suite reads it from `dmesg -s kernel` (`MIND CORE KERNEL: TIMED WAITS IN MINUTE …`), in round 181's gate. The boot's file is written from the same records; QEMU does not read the file back.
- [ ] `logs 1` still shows it. The kernel's path to `init`'s log is unchanged, but no suite reads the line there.
- [x] The gate passes: round 181, 64 groups of 64 (10f8602b4c). Its first candidate failed the host tests on `runtime.rs`'s ABI pin at 37; b7f5c9fdb4 set it to 38.
- [ ] On the Ryzen, the boot's file holds the minute lines. This is DRV's run.

## Related

- 000-KRN-0276: the minute line.
- 000-KRN-0283: the wake IPI sent again.
- 211-KRN-0019: the system log on the log volume.
