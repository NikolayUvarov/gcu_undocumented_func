# Ready: `effector` started with `SPAWN_WORKERS` for its benchmark (750-KRN-0159)

**From:** KRN · **To:** LANDING · **Written:** 2026-11-05 15:53 UTC · **Concerns:** 750-KRN-0159 (`init`'s part; EFF's part is 750-EFF-0016)

**Ready:** `claude/750-KRN-0159-effector-workers` 669681ac7d7d3a1f85fb6ccdd755f8023c008172

It contains `main` as of now (cde4a52c).

## What it brings

- **`init`:** `effector` starts with `SPAWN_WORKERS`, a task quota of 32 and one endpoint. The 32 is a ceiling: the kernel allows one live worker per online CPU. Its 48 MiB and its capabilities are unchanged.
- **`init` logs** `[INIT] effector: 48 MIB, 1 ENDPOINT AND WORKERS, AT MOST ONE PER CPU`.
- **The `modes` suite's `effector_grants_check`** requires that quota and that line.
- **Docs:** `docs/profile/bootstrap.md`.

## Tested, on 84bfcf4c (the later commits are `main`'s and the issue's)

- **acc-1, x86:** `services, storage, audio` passed.
- **acc-2, aarch64:** `the updater and the modes` passed when run again.
  - The first run failed before `modes`, in `update_suite`, on the race of the floor's line.
  - That race is fixed by the request for 000-KRN-0210 in this round.
