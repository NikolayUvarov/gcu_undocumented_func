# 750-KRN-0248 — The Effector agent's `stop-notice` taken from its worker

**Type:** kernel (`init`) · **Owner:** `KRN` · **Priority:** P1 · **Status:** open (2026-11-14) · **Blocked by:** — · **Main task:** [750](750-effector-agent.md), for [801](801-the-bench-stays-manageable.md) (S5) · **Roadmap:** stage V · **Constitution:** MC-2.3, MC-6.4

## Problem

EFF's request (`KRN/2026-11-14T1412Z-EFF-a-request-init-takes-the-stop-notice-from-the-agents-worker-and-d61f796c-merged.md`). Its effector suite's first run showed that `init` stopped the agent at once, with no `STOP NOTICE TO effector`.
- **Where it is refused:** `Request::StopNotice` finds the unit by `self.pids[i] == sender`.
- **Why the agent's call misses it:** the agent waits on a worker (750-KRN-0159), since its own thread never blocks, and the worker has a PID of its own.
- **What follows:** `init` answered the worker `denied`, and held no call to answer when it stopped the agent.

801-KRN-0242's `agent-step` had the same check, and was changed with it.

## Plan

- **The rule:** a `stop-notice` with `BADGE_NOTICE` is taken for the `effector` unit while it runs, whichever task sends it.
- **Why that is safe:**
  - `init` gives that badge to the agent alone (its `SLOT_LIFECYCLE`), and the agent lends a copy only to its own worker;
  - once the agent has ended, no unit runs, and the call is refused as before.
- **Other services** are unchanged: still the running unit whose PID sent the call.
- **`init.wit`'s comment** says so. It is 1.13, not landed yet, so no new version.

## Acceptance criteria

1. **EFF's `effector` suite:** its stop check runs instead of saying SKIP. When `init` stops the agent, the worker's held call is answered and `STOP NOTICE TO effector` is logged.
2. **The boot and services groups** pass as before.

## Related

750-KRN-0194 (`stop-notice`), 750-KRN-0159 (the agent's workers), [801-KRN-0242](801-KRN-0242-a-trial-confirmed-on-the-effector-servers-word.md).
