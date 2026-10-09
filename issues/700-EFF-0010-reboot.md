# 700-EFF-0010 — Reboot on the server's request, through the gateway

**Type:** service feature · **Owner:** `EFF` track (open) · **Priority:** P2 · **Status:** open · **Blocked by:** [700-EFF-0007](700-EFF-0007-gateway-service.md); [requests-KRN.md](requests-KRN.md) (the reboot badge for the gateway); the Effector server's reboot action (its tasks 74.12 and 75, part B) · **Main task:** [700](700-effector-agent.md) · **Constitution:** MC-6.7, MC-6.9, MC-7.1, MC-3.7

## Problem

- **Why it is needed.** Remote management needs a controlled restart.
- **On MIND Core.** `init` reboots only for a client with `BADGE_REBOOT`, which today only `updater` holds.
- **On the server.** Effector is adding a reboot operation with a guard and a check that the machine came back.

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

`idl/init.wit` (`reboot`), [700-EFF-0006](700-EFF-0006-session-service.md).
