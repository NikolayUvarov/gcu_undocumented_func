# 751-EFF-0023 — The agent installs a developer's release the server sends, while the owner's `remote-update` is on

**Type:** service, interface · **Owner:** `EFF` · **Priority:** P1 · **Status:** open (2026-10-29) · **Blocked by:** `UPD`: the gateway's client of `updater` with `install` and `status` (750-UPD-0020, extended); `KRN`: the word `remote-update` and the gateway's client (both asked on `internal-dev-messages`, 2026-10-29); the Effector server: `update_agent` for MIND endpoints · **Main task:** [751](751-system-updates-at-a-developers-request.md) · **Constitution:** MC-12.6, MC-7.5, MC-9.3, MC-3.8, MC-10.3

## Problem

The maintainer (2026-10-29): a developer agent updates a machine's system through Effector for tests, while the owner's permission is on. [751](751-system-updates-at-a-developers-request.md) gives the permission and the parts. This task is the agent's and the gateway's.

## Plan

1. **The command.** The server's `update_agent` for a MIND endpoint names an uploaded bundle by its size and SHA-256 (asked of the server). The agent:
   - journals it as any command;
   - asks the gateway whether it may proceed.
2. **The gateway decides,** in this order:
   - the owner's policy's `update` line (`apply`);
   - then `updater`'s answer, since `updater` asks `init` for `remote-update` itself.

   A refusal is audited and answered `denied`, before a byte is downloaded.
3. **The download.**
   - The agent fetches the bundle from `/api/commands/file` in pieces, as `deliver_file` does.
   - The gateway streams each piece to `updater` (`install` on its client). Nothing of it lands in a file the gateway or the agent writes.
   - The size and SHA-256 are checked at the end, as for a delivery.
4. **The apply.** `updater` installs and applies with a trial boot. The gateway answers when the restart is asked of `init`.
5. **After the restart:**
   - the agent reads `updater`'s `status`;
   - it answers the journaled command `completed` with `stdout` `updated VERSION, slot S confirmed`;
   - or it answers `failed` with the fall-back and `updater`'s reason;
   - the answer is kept until the server takes it.
6. **The heartbeat's `update` field** (contract 1.4) gives `updater`'s state: idle, receiving, staged, trial, confirmed, fell back, with the running, staged and last-known-good versions.
7. **The mark on the screen** (750-EFF-0015) is red while `remote-update` is on. The gateway reads the word from `init`.
8. **The interface.** `effector.wit` gains the update action, and the contract goes to 1.4 in [protocol.md](../docs/effector/protocol.md).
9. **The tests:**
   - the gateway's host test: the policy's refusal, `updater`'s refusal, a size or hash that differs;
   - the `effector` QEMU suite, with `tests/updater_stub` standing in for `updater` until the real one serves the gateway's badge: a release received, applied, answered after the restart; refused with the word off.

## Acceptance criteria

- [ ] Host tests as above.
- [ ] The `effector` QEMU suite, x86 and aarch64:
  - with the word on, a release from the fixture is installed and the command answered with its version after the restart;
  - with it off, the command is `denied` and nothing is sent to `updater`.
- [ ] On the DEXP and the MacBook Pro, through hesere: a developer's release installed and confirmed. Evidence for each machine only (MC-12.1).

## Related

[751](751-system-updates-at-a-developers-request.md), [750-EFF-0009](750-EFF-0009-system-update.md) (the owner's channel), 750-EFF-0021 (an answer kept until the server takes it), [750-UPD-0020](750-UPD-0020-the-gateways-client-of-the-updater.md).
