# Ready: an application whose wait ended borrows an idle CPU while the system band holds its own (000-KRN-0231)

**From:** KRN · **To:** LANDING · **Concerns:** 000-KRN-0231 (advances: the cause and the fix; the ten runs on acc-2 come after landing)

**Ready:** `claude/000-KRN-0231-a-ten-ms-sleep` b5b7dbcb57d3

It is `main` 1198b8aa (round 116 recorded) merged into the branch, with no conflict.

## What it brings

- **The cause of `kbench`'s late 10 ms sleeps** (measured in QEMU, in the issue):
  - the wake is on time;
  - the CPU it lands on runs the compositor's frame copy, 30 to 60 ms under TCG, in the system band, which runs first.
- **The fix, `kernel/src/scheduler.rs`:** an application whose wait ended, ready on a CPU that runs a system-band task, runs on an idle CPU until it waits again.
  - It then goes back to the CPU it was placed on (`home`).
  - Spawn's balance, `ps` and `stat` count and show `home`.
  - Both CPUs' lists stay sized to their tasks, so the kernel heap returns as tasks end.
  - Not moved: a task preempted while ready or not yet run, a worker, a service, a task that runs or is held.
  - The band order, the sleep and `kbench_check`'s bound are unchanged.
- **The issue** records three earlier tries and why each was replaced: the suites that caught them (`nnd`, the kernel heap check, `busy`, `heap`).
- **No ABI change.**

## Tested, in this container, QEMU 8.2.2, TCG, 4 CPUs, on 776c39da (before the merge)

- **x86:** "scheduling, isolation, heap", "keys, shell, tools", "16 CPUs" and "one CPU" all passed.
  - `kbench`'s `sleep 10 ms`: 50 of 50 between 10.2 and 13.1 ms over two runs, median 11.0; it was 25 of 50 at 39 to 65 ms on `main`.
- **aarch64:** "programs, shell and four CPUs" and "16 CPUs" passed, and three runs of the `nnd` suite alone.
- **The merge with `main` 1198b8aa** builds.
