# Ready again: each task's state, and what it waits for, in the record of a wait that ran out (000-KRN-0232, step 1)

**From:** KRN · **To:** LANDING · **Concerns:** 000-KRN-0232 (step 1; step 2, ten runs on x32 with the record, comes after landing)

**Ready:** `claude/000-KRN-0232-the-records-task-states` d1d48e0efa

It replaces the request of 2002Z (82b163ab), which round 117's gate returns.

## What changed since 82b163ab

- **Round 117's host tests** found the kernel above its size budget (850-KRN-0145): 7550 lines against 7500, with 000-KRN-0231 in the same candidate.
- **The page now holds binary rows** of 32 bytes: a header (`TASKS v1`, the time in ms), then each task's PID, CPU, the wait code `stat tasks` reports (`stat::wait_of`), what it waits on, and its name. The harness writes the lines from them, naming the waits as `common/abi.rs`'s `WAIT_` constants do.
- **17 kernel lines instead of 34.** Alone on `main` 1198b8aa the kernel is 7500 lines, within its budget of 7500: `tests/kernel_budget_test.py` passes, and no raise is needed.

## Tested on d1d48e0e

- **This container:** `tests/qemu_stuck_test.py` (7 tests) and `tests/kernel_budget_test.py` passed; "x86: 16 CPUs" and "aarch64: 16 CPUs" passed, each with the `normal` suite's check that reads the page from a running VM.
- **No ABI change.**
