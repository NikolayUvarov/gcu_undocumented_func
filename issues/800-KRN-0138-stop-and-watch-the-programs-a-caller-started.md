# 800-KRN-0138 — The loader ends the programs it started for a caller, and sends their exit notices where the caller asks

**Type:** `loader`, `kernel` · **Owner:** kernel session · **Priority:** P1 · **Status:** made, in `fast-test` (2026-10-21); the gate is left · **Blocked by:** — · **Main task:** [800](800-mind-link.md) (the link track's) · **Constitution:** MC-3.11, MC-10.2, MC-12.4

## Problem

The link track asked for two grants on 2026-10-20 (`internal-dev-messages`, `KRN/2026-10-20T2009Z-LNK-requests-numbered-and-two-grants-for-programs.md`), for 800-LNK-0009 (RUN, STOP) and 800-LNK-0011 (`effector485`'s cancel):
- **Ending a program `link` started.** `loader` is its parent, so `link` cannot end it: TASK_KILL needs process control or a descendant.
- **Its exit reason.** TASK_WATCH is the parent's only, and EXIT_STATUS needs process control. So `link` learns that a program ended (`process::alive`), not how.
- Either form that reaches only the programs `link` started will do.

A second gap came out of the design: a watch set right after SPAWN loses the notice of a program that ends at once.
- TASK_WATCH answers a child that has already ended only while its task entry is there (issue 201).
- The boot CPU's idle loop reclaims an ended task soon after, so a short program can be gone before its launcher watches it.

## Plan

1. **`loader.wit` 1.11, two calls.** A minor version: older clients are unchanged (MC-12.4).
   - `watch(session, notices)`: the exit notice of the program the session starts goes to `notices`.
     - The endpoint must carry the right to receive, as TASK_WATCH asks (`rights` otherwise).
     - The loader keeps its copy until the commit, then sets TASK_WATCH as the program's parent and drops it.
     - The caller reads the notice as a watch's: msg[1] = MSG_FLAG_EXIT, data = [PID, reason].
   - `stop(pid)`: the loader ends `pid` if it started it for this caller (`run` or a commit), and refuses any other task with `not-found`.
     - It keeps the last 64 such programs by caller.
     - It ends one with TASK_KILL, which the kernel allows a task for its descendants (issue 170).
   - A helpers-only client (109-KRN-0117) gets `rights` for both.
2. **The kernel keeps the spawner's PID** beside each of the last `EXIT_STATUSES` tasks that ended.
   - TASK_WATCH of a child that ended and was reclaimed answers from there at once, as it does before the reclaim.
   - No ABI change: the call, its arguments and the notice are as before.
3. **`check`'s row "launch: stop and notice"** (the tools suite requires it):
   - `check --child`, watched, is stopped and its notice says killed;
   - `check --exit 3`, which ends at once, gives its notice with code 3;
   - `stop` of PID 1 and of the ended child is refused with `not-found`;
   - a send-only endpoint for `watch` is refused with `rights`.
4. **Not chosen:** a capability that ends one task, answered by `commit`. It needs a new kernel capability kind, so an ABI change, for the same reach.

## Acceptance criteria

- [ ] `check`'s row "launch: stop and notice" passes in the tools suite, x86 and aarch64, in the gate.
- [ ] The link track's STOP and exit reason (800-LNK-0009) use `stop` and `watch` in its `link` suite.
- [ ] The gate passes.

## Related

[800-KRN-0132](800-KRN-0132-link-starts-programs-for-the-master.md) (`link`'s loader client), [800-KRN-0137](800-KRN-0137-link-may-make-one-endpoint.md) (its endpoint), 800-LNK-0009, 800-LNK-0011, [docs/profile/bootstrap.md](../docs/profile/bootstrap.md), [docs/api/README.md](../docs/api/README.md) (TASK_WATCH).
