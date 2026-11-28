# 000-KRN-0293 — The boot log's journal stops saving in the USB image check, twice in a gate

**Type:** `vfs_server` journal, `logd` · **Owner:** `KRN` (the kernel session) · **Priority:** P1 · **Status:** open (2026-11-28) · **Blocked by:** — · **Main task:** — · **Constitution:** MC-10.6, MC-12.1

## Problem

"x86: USB image" failed in two landing gates. Each time it passed when run again alone.
- **Round 177** (2026-11-26, on x32-2, 98 s).
- **Round 183** (2026-11-28, on i9, 59 s).

**What failed.** `tests/usb_image_smoke.py`, the log partition's check (211-KRN-0019):
1. `logger LOG-PARTITION-CHECK` answered `LOGGED`, and `sync` answered `OK`.
2. `cat log:boot0001.log`, read 20 times over 10 s, held the file but never the line.

**Why it is not a short wait.**
- In round 183 the saved file ends at second 8.150, the second block store's `READY`.
- The check ran long after that. In the boot's first two minutes the journal saves every record within `SAVE_MS`, 2 s, and `sync` saves everything (211-KRN-0223).
- So the journal stopped saving after 8 s, or `sync` did not reach it.
- On a machine, that would lose a boot's log from its first seconds on, the case 211-KRN-0019 exists for.

## Plan

1. **Collect what the failed runs left:** the saved files of rounds 177 and 183 on x32-2 and i9 (`logs/r177/x32-2-10`, `logs/r183/i9-06`), and `dmesg -s vfs_server` at the failure.
2. **Make the check say more when it fails:** `logd`'s record count, the journal's last save and its state, before it asserts.
3. **Repeat the group** beside a busy lane until it fails again, or ten runs pass.
4. **Mend the cause** in the journal or in `logd`, with a host test where the cause allows one.

## Acceptance criteria

1. The cause is named from a failed run's own lines, or ten runs beside a busy lane pass and the check's new lines are kept for the next failure.
2. If mended: the group passes ten times beside a busy lane on x32, and the gate's other groups that read the log volume pass.

## Related

211-KRN-0019 (the log partition), 211-KRN-0223 (the cadences and `sync`), 000-KRN-0254 (the same group's earlier mends).
