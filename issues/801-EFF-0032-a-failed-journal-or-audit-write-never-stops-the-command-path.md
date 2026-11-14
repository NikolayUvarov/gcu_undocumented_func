# 801-EFF-0032 — A journal or audit line not written never stops the command path: the service ends, and init starts it again

**Type:** session, gateway · **Owner:** `EFF` · **Priority:** P1 · **Status:** in progress (2026-11-14) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S5: only a deliberate switch-off stops the agent) · **Constitution:** MC-6.6, MC-10.6, MC-6.5

**Reported by:** LNK, `EFF/2026-11-14T1144Z-LNK-801-s5-the-macs-agent-stops-taking-operations-while-its-telemetry-goes-on.md`.

## Problem

**What hesere showed for the MacBook Pro** (`mind_353abc18de2dbd0e`, release 202611140614) at 12:30 UTC on 2026-11-14:
- online, with heartbeats and telemetry flowing;
- `busy: true`, `occupied_queues 1`, but `last_command_poll` 11:34:55: no poll for 55 min;
- the `collect_logs` taken at 11:34:55 ended `timeout`, and the two after it `expired` in the queue;
- telemetry's `executed_requests` 0 for this boot. The agent counts a command only once its journal line is written, so that line was not.

**Why the agent stayed like that** (`effector/src/session.rs`). One journal write that fails sets `journal_failed`, as MC-6.6 asks: no answer leaves before it is kept. Then:
- `next_ack` and `poll` give nothing, for good;
- `busy()` stays true;
- the heartbeats go on saying `busy`;
- 801-EFF-0028 does not see it, since the server still answers.

The test of that rule names it "until recovery", but only a hand at the machine recovered it.

**The likely cause, not yet read from the Mac's log:** the agent's private directory and the gateway's audit are scopes `init` asked of one `vfs_server` instance. After a restart of `vfs_server`, the new instance does not know them, so every write through them fails. The gateway is in the same state: its audit line is not written, so it refuses every audited request (MC-10.6), for good.

## Plan

- **The rule stays:** nothing is answered that is not journaled (MC-6.6), and nothing is done that is not audited (MC-10.6).
- **The recovery is automatic:**
  - **the agent**, once its journal was not written, logs why (open, header, write or flush, with the error) and ends. `init` starts it again with new clients, and its journal is read back.
  - **the gateway**, once an audit line was not written, answers that request `audit` as before, logs it and ends. `init` starts it again.
  - **A cause that stays** (a disk that refuses every write) gives one restart per request. 801-KRN-0245 starts a quarantined service again after a cool-down, so the agent is never gone for good.
- **The Mac's log of the stop,** once the cable is back on it (LNK), confirms or corrects the cause.

## Acceptance criteria

- [ ] **The `effector` suite, x86 and aarch64,** with `vfs_server` restarted under the agent and the gateway: a command after it is done, directly or after the services' own restarts, and a log is collected through the gateway's audit.
- [ ] **On the MacBook Pro, through hesere:** after hours, and after a `vfs_server` restart, commands are taken and answered with no hand at the machine (MC-12.1).

## Related

[801](801-the-bench-stays-manageable.md), [801-EFF-0028](801-EFF-0028-the-agent-comes-back-by-itself.md), [750-EFF-0006](750-EFF-0006-session-service.md).
