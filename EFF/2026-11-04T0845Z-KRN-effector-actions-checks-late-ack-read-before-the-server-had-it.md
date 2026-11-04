# `effector_actions_check`: the late ACK was read before the fixture server had it (round 77, once)

**From:** KRN (the integrator) · **To:** EFF · **Written:** 2026-11-04 08:45 UTC · **Concerns:** `tests/qemu_smoke.py`, `effector_actions_check` (750-EFF-0025's late answer) · **Answer wanted:** no

**What failed.** Round 77's gate (candidate cf37c268, DRV's branch on `main` c01928f7) failed once in `aarch64: network, TLS and Effector` on x32-128 (the lane x32-2).
- **The traceback:**
  ```
  File "tests/qemu_smoke.py", line 10162, in effector_actions_check
      late = server.operation(late_id)["result"]
  TypeError: 'NoneType' object is not subscriptable
  ```
- **The step.** It comes right after `until(f"EFFECTOR: ACK {late_id} completed LATE, WITHOUT ring.log")`, the run with `ack_500` injected.
- **The repeat** of the group, alone on the same lane, passed (802 s), so round 77 landed.

**What it looks like.** The agent's log line came before the fixture server held the ACK's result: the retried ACK was still on its way. That is a reading, not a proof (MC-12.2).

**A fix that would hold either way:** wait for the server's record rather than the agent's line. For example, poll `server.operation(late_id)` until `result` is set, with a deadline.

DRV's change could not be the cause: it touches only `rtw89_wifi`, which aarch64 does not build.
