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

- [x] **The `effector` suite, x86 and aarch64,** with `vfs_server` restarted under the agent and the gateway: a command after it is done, directly or after the services' own restarts, and a log is collected through the gateway's audit. — LEG9, 2026-11-15, `ec86ae95`: PASS on x86_64 and aarch64; and the boot disk gone under the agent, with 801-KRN-0251: heartbeats went on, a `collect_logs` refused `journal` and not run, a reboot answered `completed`, and `init` restarted the machine.
- [ ] **On the MacBook Pro, through hesere:** after hours, and after a `vfs_server` restart, commands are taken and answered with no hand at the machine (MC-12.1).

## Related

[801](801-the-bench-stays-manageable.md), [801-EFF-0028](801-EFF-0028-the-agent-comes-back-by-itself.md), [750-EFF-0006](750-EFF-0006-session-service.md).

## Progress (2026-11-14): the cause corrected, and the plan revised

**LNK's correction** (`EFF/…-LNK-801-s5-correction-the-stops-came-with-the-boot-disk-taken-out.md`):
- the Mac's agent stopped when its boot disk was taken out for a rewrite while the system ran;
- the DEXP's did the same at 14:00 UTC.

**Why the first form was wrong for that case:** with the disk gone, an agent that ends cannot read its `effector.conf` when `init` starts it again. So the machine leaves the server's view as well as its control.

**The revised form** (`docs/effector/protocol.md`, "When the journal or the audit cannot be written"):
- **A restarted `vfs_server`** (another PID in `init`'s list than at the start): the agent, or the gateway for its audit, ends as before, for new file clients.
- **The same `vfs_server`:** both go on, the heartbeats too.
  - Every command is answered at once, `failed` with the new code `journal`, neither journaled nor run.
  - The answer the journal failed to keep is still sent.
  - `reboot`, under the owner's `reboot on`, is answered `completed` and asked of `init`.
  - The gateway refuses every audited request but that restart, whose audit line is in the system log alone (MC-10.6's mode, stated before it is needed).
- **`session.rs`:**
  - polls and ACKs go on with the journal failed;
  - `busy` no longer counts the failure;
  - `receive` still refuses every new command, so nothing is ever run without its record.

**Tests:**
- **Host:** the session's journal-failure case and the gateway's `with_the_audit_unwritable_only_the_restart_goes_on`.
- **The `effector` suite:** the boot disk's driver is stopped under the agent in the last boot. Heartbeats go on, a `collect_logs` is refused `journal`, and a `reboot` is answered.

**What `init` does with that reboot is KRN's:** it reads the owner's policy from the disk at each request, so with the disk gone it refuses the restart. Asked of KRN: keep the last policy read, for this case.

