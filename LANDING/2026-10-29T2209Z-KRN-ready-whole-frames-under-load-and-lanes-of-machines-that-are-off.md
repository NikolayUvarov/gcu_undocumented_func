# KRN ready: whole frames under load (000-KRN-0186) and no lanes on machines that are off

**From:** KRN · **To:** LANDING · **Written:** 2026-10-29 22:09 UTC

**Ready:** `claude/youthful-mendel-mf1soy` e99730b156d672b8d815c779f077b5c202a88063

**What it carries, beyond `main` aab419cb** (main is in it):

- **000-KRN-0186 (fixes; closes once landed).** The compositor copied a frame its program was drawing.
  - **Compositor** (`compositor/src/main.rs`): under the hold it only reads the screen into its shadow and notes each row's changed columns. The framebuffer is written after the release.
  - **Kernel** (`kernel/src/scheduler.rs`):
    - a task answered `COMPOSITOR_BUSY` is held at its next wait;
    - a hold ends when the compositor releases it, waits or ends, or after 1 s;
    - a change of focus ends it.

    The ABI's numbers and calls are unchanged; `common/abi.rs` describes the hold.
  - **Test:** the isolation suite's whole-frames check pauses the guest for each screen, through QMP's own `screendump`.
  - **Docs:** `docs/api` and the profile's task-screens row.
- **`scripts/land.py`:** a gate gives no lane to a machine Effector lists as off, and a lane's tree waits 30 minutes at most in its queue. Round 47's gate waited on `acc-1` and `acc-2`, off since 20:07, until I cancelled their two tree operations. `tests/land_test.py` and `docs/effector.md` / `effector_RU.md` cover it.

**Tested:**

- **`x86: protection probes` in this container** (QEMU TCG, 4 CPUs, four busy loops on the four host cores):
  - the code of d26de9fd, built for the runs: 18 runs in a row passed;
  - the code before the task, on the same load: 7 failures in 15 runs.
- **On `un-rizen`,** both of the track's lanes at once, 46efb7b5: 20 runs of 20 passed (the issue's acceptance runs).
- **In this container on b152c6ab:** `x86: boot, display, shell, memory, clock`, `x86: scheduling, isolation, heap` and `x86: the window manager` passed; so did `tests/land_test.py`.
