# 750-APP-0093 — The shell's `effector` command: setup, policy, status

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P3 · **Status:** open (planned) · **Blocked by:** `EFF`: [750-EFF-0003](../issues-done/750-EFF-0003-interface-and-policy.done) (the interface and the policy), [750-EFF-0006](750-EFF-0006-session-service.md) (the session service); `svc enable` ([173-APP-0092](173-APP-0092-svc-boot.md)) · **Main task:** [750](750-effector-agent.md) · **Roadmap:** — · **Constitution:** MC-3.11, MC-7

**Recorded by:** the Effector agent track (EFF), 2026-10-09, in `requests-APP.md`, for main task 750 at the maintainer's request. Numbered by the tools track on 2026-10-20, at the maintainer's word.

## Problem

The Effector agent is off by default, and only the owner may turn it on and say what the server may do. Only the shell can write to `data/`, so the owner's commands for the agent belong in the shell.

## Plan

- **`effector setup URL PIN [NEXT-PIN]`:**
  - writes the server's address and certificate pins to `data/effector/`;
  - reads the token from the console without echoing it;
  - asks for confirmation and shows what it wrote, except the token.
- **`effector policy show | edit | allow <line> | deny <line>`:** the policy of 750-EFF-0003, with confirmation for anything that widens it.
- **`effector enable | disable`:** through `svc enable`/`disable` for both services; disabling also revokes the session.
- **`effector status`:** connected or not, the server and its pin, the last heartbeat accepted, the operations in flight, the last audit lines.
- **The same commands in `msh`** under `requires: lifecycle`, and help in `docs/tools/`.

## Acceptance criteria

In QEMU, the shell sets up the agent, enables it, and the 750-EFF-0006 suite sees it online. `policy deny` takes effect on the next request; `disable` stops both services.

## Related

[750](750-effector-agent.md), [750-EFF-0003](../issues-done/750-EFF-0003-interface-and-policy.done), [750-EFF-0006](750-EFF-0006-session-service.md).
