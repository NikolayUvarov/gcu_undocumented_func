# 751-EFF-0023 — The agent installs a developer's release the server sends, while the owner's `remote-update` is on

**Type:** service, interface · **Owner:** `EFF` · **Priority:** P1 · **Status:** in progress (2026-10-30) · **Blocked by:** the Effector server: `update_agent` for MIND endpoints (the hardware criterion only) · **Main task:** [751](751-system-updates-at-a-developers-request.md) · **Constitution:** MC-12.6, MC-7.5, MC-9.3, MC-3.8, MC-10.3

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

- [x] Host tests as above.
- [x] The `effector` QEMU suite, x86 and aarch64 (its `effector-update` part, a group of its own; the refusal is answered
  `failed` with `denied`, as the gateway's other refusals are):
  - with the word on, a release from the fixture is installed and the command answered with its version after the restart;
  - with it off, the command is `denied` and nothing is sent to `updater`.
- [ ] On the DEXP and the MacBook Pro, through hesere: a developer's release installed and confirmed. Evidence for each machine only (MC-12.1).

## Progress (2026-10-30)

**Done on `claude/EFF-effector-agent`** (the parts it waited for are in `main`: 751-KRN-0185, 751-UPD-0032):
- **The interface:** `effector.wit` 1.5, `update-begin`, `update-commit`, `update-apply` and `update-state`, after
  `connection` so the older calls keep their numbers. The existing `write`, `close` and `cancel` serve the bundle too.
- **The gateway** (`effector_gw/src/gateway.rs`, `main.rs`), in this order, each step audited:
  - the policy's `update apply`;
  - the owner's `remote-update` as `init` says it (`init.wit` 1.11 `mode`): with it off nothing reaches the updater;
  - the updater's `install-begin`, which asks `init` again itself;
  - the bundle's pieces, in order, to `install-write`; nothing lands in a file the gateway writes;
  - `install-commit`, then `apply` on its own call.
- **The agent** (`effector/src/main.rs`, `update.rs`, `wire.rs`): `update_agent` with `size`, `sha256`, `version`,
  `tries` and `timeout_seconds` (default 1800). A refusal is answered at once, before a byte is downloaded. Once the
  release is staged, the command is kept in `system/effector/update`, flushed, and the release applied. After the
  restart the agent answers from the updater's view:
  - `completed`, `updated VERSION, slot S confirmed`, once `init` confirmed the trial boot;
  - `failed` with `fell-back` (and the updater's last failure), `not-confirmed` after 180 s, `restart` if the restart
    did not come within 60 s, or `io`.

  A `version` other than the one staged is `failed` (`version`), not applied.
- **Contract 1.4** in [protocol.md](../docs/effector/protocol.md), and [README.md](../docs/effector/README.md).
- **Tests:**
  - host: `tests/effector_gateway_host.rs` (the policy's refusal, `remote-update` off, the updater's refusal, a short
    or changed bundle, the order of the pieces, the happy path, each step audited), `tests/effector_update_host.rs`
    (the kept command, the restart, the verdicts), `tests/effector_wire_host.rs` (the payload);
  - the fixture serves an `update_agent` bundle and takes the late ACK (`tests/effector_server_test.py`);
  - QEMU: the `effector-update` suite, a CI group of its own (x86 and aarch64): the agent and the fixture of the `effector`
    suite on a slot image, release 5 in slot A, release 6's bundle from `release.py bundle`, the real updater. PASS on
    both, 2026-10-30, on LEG9 (QEMU TCG under WSL):
    - with `remote-update` off: `failed` with `denied`; nothing reached the updater, nothing was downloaded or staged,
      no boot record written;
    - with it on: the bundle (23.6 MB on x86, 23.7 MB on aarch64) downloaded into the gateway and streamed to the
      updater, release 6 staged in slot B and applied, the machine restarted into slot B on trial; after `init`
      confirmed it the agent answered the kept command `completed`, `updated 6, slot B confirmed`. From the download's
      start to the trial boot: 39–41 s on x86, 385 s on aarch64.

    The `net,tls,effector` groups pass on both with these changes. This is QEMU evidence only (MC-12.1).

**Checked again after `main` was merged (14487ec6, 2026-10-30, LEG9, `scripts/ci_local.sh`):**
- all host tests (`scripts/host_tests.sh`, with OpenSSH's server for `ssh`): 165 suites pass;
- the groups that the gateway's new grants touch:
  - `x86: services, storage, audio` (`modes` among them);
  - `x86: the updater and an update through the link` (`updater` with `install`, and `linkupdate`);
  - `aarch64: the updater and the modes`.

  All pass.

**Open:**
- **The hardware criterion:** the Effector server's `update_agent` for MIND endpoints, now its task `81-mindcore-08-update-agent-release-bundle` (sst-test-deploy 9bc793f). Its
  field names, once fixed, replace the ones above.
- **Plan item 6,** the heartbeat's `update` field: waits for the server's field.
- **Plan item 7,** the mark: red already whenever the policy says `update apply` (a change past a check); a red mark
  for `remote-update` alone, with no policy line, is not done.

## Related

[751](751-system-updates-at-a-developers-request.md), [750-EFF-0009](750-EFF-0009-system-update.md) (the owner's channel), 750-EFF-0021 (an answer kept until the server takes it), [750-UPD-0020](750-UPD-0020-the-gateways-client-of-the-updater.md).
