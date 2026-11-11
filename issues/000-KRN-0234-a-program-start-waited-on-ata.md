# 000-KRN-0234 — A program's start waited past 8 s, the chain ending at `ata` (round 121, x32)

**Type:** tests (the stuck record) and the boot disk's path · **Owner:** `KRN` · **Priority:** P2 · **Status:** open (step 1 in `main` since round 122, 33c38b60; step 2, ten runs on x32 beside a busy lane, started) · **Blocked by:** — · **Main task:** — · **Roadmap:** track A · **Constitution:** MC-12.2

## Problem

**Round 121's gate,** candidate 1a18628c (`main` f8a5220a with 110-NET-0068), "x86: RAM above 4 GiB" on x32 (x32-128). In the `net` suite's `browse_check`, `run netcheck ask:11.0.2.80:81:30 &` got no prompt in 8 s.
- **The third such start in that group on x32,** after rounds 114 and 116 ([000-KRN-0232](../issues-done/000-KRN-0232-a-program-start-sent-nothing-in-8-s-in-the-wifi-check.done), closed: the next one opens a new issue).
- **The record** (000-KRN-0214 with 000-KRN-0232's task states), at 20 000 ms, is kept on x32-128 in `mind-core/KRN/logs/land-r121/x32-03/`:
  - all four CPUs halted;
  - the shell waits for a reply from the loader (PID 10), the loader from `vfs_server` (PID 9), and `vfs_server` from `ata` (PID 6);
  - `ata` shows as ready, and so does the compositor. The page is written in the tick that wakes sleeping tasks, so a task that sleeps between polls shows as ready there: it is not proof of a lost wakeup.
- **The host:** load average 28 over 5 minutes, on its 32 CPUs.
- **`ata`'s waits** poll the drive for 2 ms, then between 1 ms sleeps, to a deadline of 10 s for each sector (000-DRV-0064, in `main` since round 113, before the first of the three).

## What was ruled out

**`ata`'s sleeps do not slow a start** (this container, 4 CPUs, QEMU 8.2.2 under TCG, `main` f8a5220a with 110-NET-0068). Eighteen programs were started once each, so that each ELF is read from the IDE disk, and the time from the command to the prompt summed:

| `ata` | Idle | Four busy loops on the 4 CPUs |
|---|---|---|
| as in `main` | 5.3 s | 15.5 s, and 15.7 s again |
| polling without sleeps to its deadline, as before 000-DRV-0064 | 6.4 s | 15.3 s |

The slowest single start under load was 1.3 to 1.4 s with either. What a loaded x32 does to the drive's own reads (the host's disk) is not reproduced here.

## Plan

1. **The stuck record tells the drive from the driver.** When a wait runs out, it adds:
   - QEMU's `info blockstats`, twice, a second apart: whether the drive's reads advance;
   - the task-states page, twice, a second apart: whether `ata` goes on waking between polls.
2. **Reproduce:** "x86: RAM above 4 GiB" ten times on x32 beside the other lane, with the record.

## Acceptance criteria

- The cause is named, with the record that shows it, or ten runs on x32 beside a busy lane pass with the record added, which then stays.

## Progress

**Step 1 (2026-11-11),** `tests/qemu_smoke.py`: the stuck record holds the tasks' states and `info blockstats` (each drive's operations, bytes and total time) twice, `STUCK_AGAIN` (1 s) apart.
- **`tests/qemu_stuck_test.py`:** both readings in the record, in the monitor commands' order, and the drives' section when the kernel printed no address.
- **In a VM here** (x86, QEMU 8.2.2): the record of a probe held `ide0-hd0`'s counts twice and the tasks twice.

**Step 1 landed** in round 122 (`main` 33c38b60, 2026-11-11); the gate passed every group on its first run. Round 121's group passed when the gate ran it again alone (1254 s). **Left:** step 2, ten runs of "x86: RAM above 4 GiB" on x32 beside x32-2's runs, with the record.

## Related

[000-KRN-0232](../issues-done/000-KRN-0232-a-program-start-sent-nothing-in-8-s-in-the-wifi-check.done), [000-KRN-0214](../issues-done/000-KRN-0214-the-shell-stopped-answering-during-the-effector-screenshot.done) (the record), [000-DRV-0064](000-DRV-0064-ata-and-ahci-waits-with-a-deadline.md) (`ata`'s waits).
