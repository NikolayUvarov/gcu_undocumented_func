# 750-EFF-0010 — Reboot on the server's request, through the gateway

**Type:** service feature · **Owner:** `EFF` track · **Priority:** P2 · **Status:** open · **Blocked by:** [750-EFF-0007](750-EFF-0007-gateway-service.md); [requests-KRN.md](requests-KRN.md) (the reboot badge for the gateway); the unverified server reboot-action/return-to-online contract retained as an open requirement in this issue · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-6.7, MC-6.9, MC-7.1, MC-3.7

## Problem

- **Why it is needed.** Remote management needs a controlled restart.
- **On MIND Core.** `init` reboots only for a client with `BADGE_REBOOT`, which today only `updater` holds.
- **On the server.** A typed reboot operation needs an authorization guard and a check that the machine came back. This audit has not established its current server contract or replacement external task number; historical task 75 is unrelated console work. Neither the prepared platform boundary of external task 81 nor its 81-01 agent/live follow-up implements or verifies this reboot feature. The requirement remains open here; no remote machine is rebooted to investigate it.

## Plan

- **The typed `reboot` call (0003),** with a delay and the reason. It is allowed only if the policy says so.
- **The gateway asks `init` to reboot,** after `effector` has sent a heartbeat with `system_state: "shutting_down"`.
- **After the restart:**
  - the heartbeat carries the boot time, so the server can tell the machine came back;
  - the reboot request's result comes from the journal.
- **When the server's action does not exist yet,** nothing is added to the contract.

## Acceptance criteria

- **In the `effector` suite:**
  - an allowed reboot restarts the guest and the agent is online again with a new boot time;
  - a reboot the policy forbids is `denied`;
  - a repeated command ID does not reboot twice.

## Related

`idl/init.wit` (`reboot`), [750-EFF-0006](750-EFF-0006-session-service.md).

## Progress — 2026-10-23, the server's `exec` of `reboot`

- **The trigger.** The Effector server has no reboot operation, but it sends its `exec` as `exec_v1`. At the maintainer's word ("выполнение команды reboot не достаточно?"), the agent maps exactly the command `reboot` to the gateway's `reboot`. Every other command stays denied.
- **The gateway** admits it only under `reboot on` and audits it. It answers, then asks `init` 10 s later through its lifecycle client (750-KRN-0154). `init` checks the owner's policy again.
- **Meanwhile the session** keeps its answer and sends it, `completed`. If the ACK does not get out before the restart, the journal sends it after.
- **Tested:** the gateway's host tests; the `effector` suite's refusal under a policy without `reboot on`. The restart itself is to be seen on the MacBook, once its owner adds `reboot on`.
