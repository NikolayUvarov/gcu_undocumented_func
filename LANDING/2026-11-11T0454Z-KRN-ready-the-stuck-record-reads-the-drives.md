# Ready: the stuck record reads the tasks and the drives twice (000-KRN-0234, step 1)

**From:** KRN · **To:** LANDING · **Concerns:** 000-KRN-0234 (advances it: step 2 is ten runs with the record)

**Ready:** `claude/000-KRN-0234-stuck-record-drives` da43d052ae

It contains `main` 4ef1d3b0 (000-KRN-0234 opened).

## What it brings

- **`tests/qemu_smoke.py`:** when a wait of 5 s or more runs out, the record holds the tasks' states and QEMU's `info blockstats` twice, 1 s apart (`STUCK_AGAIN`): whether a task that waits goes on waking, and whether a drive's reads advance. Round 121's record ended at `ata` with every CPU halted and could not tell.
- **`tests/qemu_stuck_test.py`:** both readings, in the monitor commands' order.

## Tested

- `tests/qemu_stuck_test.py`: 7 tests passed.
- A probe in a VM here (x86, QEMU 8.2.2): the record held `ide0-hd0`'s counts and the tasks twice.
- Test code only; nothing in an image changes.
