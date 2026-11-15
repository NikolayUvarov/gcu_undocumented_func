# 801-UPD-0043 — A release the updater refuses, sent through the agent, leaves the machine on its slot and connected, the refusal named to the server

**Type:** update (tests: the `rollback` QEMU suite) · **Owner:** `UPD` (the kernel session) · **Priority:** P1 · **Status:** in progress (2026-11-15) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S4) · **Roadmap:** stage V · **Constitution:** MC-9.2, MC-9.4, MC-12.1

## Problem

801's S4 says: "A release the running updater refuses leaves the machine on its slot, connected, and the refusal reaches the server."

**What the code does already:**
- The agent streams an `update_agent` bundle into the updater through the gateway (751-EFF-0023).
- When the updater refuses it at the commit, the agent answers the command failed: `not staged: <reason>` (`effector/src/main.rs`, `update_agent`).

**What no check shows:**
- The `update` suite refuses a bad signature, an older version and a changed boot file, but from a source the updater reads itself, not through the agent.
- The `effector-update` suite refuses a bundle only at the gateway, with `remote-update` off, before a byte is downloaded.
- So nothing shows a bundle downloaded whole, refused by the updater, the command answered with the reason, and the machine still on its slot and on the server.

## Plan

1. **In the `rollback` suite, before release 6 is sent:** an `update_agent` command with release 6's bundle whose channel signature is changed.
2. **The check:**
   - the agent logs `not staged: signature`;
   - the server's result is failed, with that reason;
   - `update status` names the same running slot, boot record and nothing staged as before the command;
   - a heartbeat is accepted after the answer.
3. Then the suite goes on as before, with release 6 as published.

## Acceptance criteria

- The `rollback` suites, x86 and aarch64, pass with the refused bundle first.
- The gate passes.

## Related

- [801](801-the-bench-stays-manageable.md), S4
- [801-UPD-0041](../issues-done/801-UPD-0041-a-release-that-loses-the-server-rolled-back.done), the `rollback` suite
- 751-EFF-0023, `update_agent`
