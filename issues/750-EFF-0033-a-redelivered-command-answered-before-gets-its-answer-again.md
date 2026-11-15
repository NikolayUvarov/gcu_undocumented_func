# 750-EFF-0033 — A redelivered command answered before gets its answer again, never `failed: io`

**Type:** session · **Owner:** `EFF` · **Priority:** P2 · **Status:** in progress (2026-11-14) · **Blocked by:** — · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-6.6, MC-12.3

**Asked by:** the Effector project's `issues-mindcore/81-01-native-contract-and-hardware.task.md`, "Post-ACK custom-result question found in QEMU".

## Problem

- **What `finished` did:** it removed the private `result` file once a measured result's ACK ended.
- **What a redelivery of that command ID got:** the journal still held the answer, but `ack_result` found no file, so it was answered `failed` with `io`. The action was not run again.
- **Why that is wrong:**
  - the command completed, and its first answer said so;
  - a later `failed` contradicts it, and a server that keeps the first result has to refuse it.

An artifact (`collect_logs`, `screenshot`) is removed the same way, with the same answer.

## Plan

- **The last measured result stays** until the next one replaces it. A redelivery of it is answered with the same bytes.
- **For an older measured result, or any artifact,** the session knows the command was answered before: its first ACK ended (`Session::answered`). The redelivery is then answered:
  - `completed`, with `exit_code` 0;
  - `stdout` saying the first answer carried the result and it is not kept for a repeat;
  - the log `EFFECTOR: ACK <id> completed AGAIN, ITS RESULT NOT KEPT: …`.

  The server keeps its first result.
- **A result lost before its first ACK ended** is still answered `failed` with `io`.
- **Documented in** `docs/effector/protocol.md`.

## Acceptance criteria

- [x] **Host test** (`tests/effector_session_host.rs`): a command is known as answered only once its first ACK ended, and stays so through a redelivery.
- [x] **The `effector` suite, x86 and aarch64, after the agent's restart:** LEG9, 2026-11-15, `d48f419c`: PASS on both.
  - the last measured command redelivered: the same result, taken;
  - an older one: completed again, not kept, and the server's first result unchanged;
  - neither run again.
- [ ] **The answer in the Effector project's tracker** (81-01), once landed.

## Related

[750-EFF-0006](750-EFF-0006-session-service.md), [750-EFF-0025](750-EFF-0025-an-ended-command-no-longer-holds-the-agent.md).
