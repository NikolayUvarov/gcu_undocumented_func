# 750-EFF-0021 — A delivery answers with the file as read back: its path, size and SHA-256

**Type:** service, interface · **Owner:** `EFF` · **Priority:** P1 · **Status:** in progress (2026-10-29) · **Blocked by:** — · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-10.3, MC-12.1, MC-12.4

## Problem

The Effector server's task `81-mindcore-04-QUESTIONS-01` (sst-test-deploy, 2026-10-24) cannot close. It needs evidence that a file delivered to the MacBook is there with its size and SHA-256, and MIND gives none.

- **What a delivery answers.** A completed `deliver_file` answers `completed` with an empty `stdout`. The answer names neither the destination nor its size or hash.
- **What the gateway checks.** It compares the bytes it received with the announced size and SHA-256 before publishing. It never reads the published file under its name.
- **What the audit line holds.** It names the path and the byte count, but no digest.
- **The other ways are closed by the owner's policy, rightly:**
  - `collect_file` of the inbox is denied, since a delivery root grants no read;
  - the remote shell is denied.

The maintainer (2026-10-29): both MIND machines are to be reachable through Effector, and Effector's problems are to be fixed.

## Plan

1. **`effector.wit` 1.4:** `deliver-commit` answers `delivered { path, size, sha256 }`.
2. **`effector_gw`:**
   - after publishing, it reads the file back under its name, below the delivery root, through the scope it wrote with;
   - it answers the size and SHA-256 it read;
   - a read-back that differs from the announced values is `digest`, and the file stays;
   - the audit's result field holds the read-back digest's first 16 hex digits.
3. **`effector`, contract 1.3** ([protocol.md](../docs/effector/protocol.md)):
   - a completed delivery's `stdout` is `delivered PATH SIZE SHA256`;
   - the agent keeps that result until the server takes it, as it keeps a measurement;
   - if the result cannot be kept, the answer is the bare `completed`.
4. **The tests:**
   - the gateway host test: the answer, and a read-back that differs;
   - the wire host test: the result's JSON;
   - the `effector` QEMU suite: the fixture's operation carries the line, with the right size and SHA-256.
5. **The server:** a task in sst-test-deploy's tracker for `81-mindcore-04-QUESTIONS-01`, once a machine runs this. A new delivery to the MacBook then answers with the line the task asks for.

## Acceptance criteria

- [x] Host tests: `tests/effector_gateway_host.rs` (the answer, a read-back that differs) and `tests/effector_wire_host.rs` (the result's JSON), 2026-10-29.
- [ ] The `effector` QEMU suite on x86 and aarch64: the delivery's `stdout` is `delivered data/inbox/d.txt SIZE SHA256` with the file's own values.
  - x86, 4 CPUs (98a23c3e, 2026-10-29): passes, with the line's size and SHA-256 checked.
  - aarch64: not run yet.
- [ ] On the MacBook Pro, through hesere: a `deliver_file` answers with the line, and the server's task is told. Evidence for that machine only (MC-12.1).

## Related

[750-EFF-0007](750-EFF-0007-gateway-service.md) (the gateway), [750-KRN-0120](../issues-done/750-KRN-0120-writable-scopes-for-a-service.done) (the delivery root's scope), sst-test-deploy `issues/81-mindcore-04-QUESTIONS-01-inbox-verification.task.md`.
