# 750-EFF-0007 — `effector_gw`: the gateway that acts on the machine

**Type:** service · **Owner:** `EFF` track · **Priority:** P1 · **Status:** open · **Blocked by:** ([750-EFF-0003](../issues-done/750-EFF-0003-interface-and-policy.done), the interface and the policy, done); [requests-KRN.md](requests-KRN.md) (boot images and `init`'s grants, a writable scoped `vfs` client, a lifecycle client limited to named services) · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-3.1, MC-3.4, MC-3.7, MC-3.8, MC-10.2, MC-10.3, MC-10.7, MC-6.8, MC-6.9

## Problem

- **What the server asks for.** The server asks to restart services, return logs and files, and put files on the machine.
- **Where it must happen.** These need authorities that `effector` must not hold.
- **How each request is decided.** By the owner's policy, and recorded.

## Plan

- **A boot service `effector_gw`, off by default.** Its grants from `init`:
  - a lifecycle client limited to the services the policy may name (requests-KRN);
  - the log read badge;
  - `sysinfo`;
  - a writable `vfs` client scoped to `data/effector/` (configuration excluded);
  - a read-only client for the policy's file roots and `log:`;
  - its own private directory in `system/`, for the command journal;
  - screen capture, given only if the policy allows it.
- **The calls of 0003, each checked against the policy, then audited.**
  - **`status`:** boot services and their states from `init.list`, memory and load from `sysinfo`, the release version, the update state.
  - **`service-control`:** start, stop or restart a named service through `init`; never `init`, the shell, `effector` or `effector_gw`.
  - **`collect-log`:**
    - the log ring, or a persisted boot log;
    - returned as text or as an artifact;
    - bounded by the size limit of the protocol.
  - **`collect-file`:** a file under a policy root, read in chunks with its SHA-256; the agent's own directory is never a root.
  - **`deliver-file`:**
    - written to a temporary name under the delivery root;
    - size and SHA-256 checked;
    - flushed, then renamed;
    - an existing file is kept unless the request allows replacing it.
  - **`screenshot`:** where the policy allows it, the screen as the shell's `screenshot` takes it (086), returned as an artifact.
  - **`test-run`:** handed to the test-account runner of 501-ASR-0009 when it exists and the policy allows test runs; `denied` until then.
  - **`cancel`:** of a transfer in progress.
- **Busy work.** A request that would block the gateway runs with its deadline and does not hold up `status`.

## Acceptance criteria

- **In the `effector` suite, against the 0002 server:**
  - a service the policy names is restarted and one it does not is `denied`;
  - a log and a file come back with the right hash;
  - a path outside the roots is `denied`, and so is the agent's own token;
  - a delivered file appears whole or not at all, also when the transfer is cut;
  - each request has its audit line.
- The `isolation` suite checks that the gateway holds only its listed grants.

## Related

[750-EFF-0003](../issues-done/750-EFF-0003-interface-and-policy.done), [750-EFF-0006](750-EFF-0006-session-service.md), `idl/init.wit`, `idl/log.wit`, `idl/sysinfo.wit`, `idl/vfs.wit`.

## Progress — 2026-10-21

The gateway's decisions and effects are built as a library, apart from IPC, so they are tested on the host before `init` can start the service (750-KRN-0119):

- **`effector_gw/src/gateway.rs`:** every request of `idl/effector.wit` goes through the badge (actions only from the session, `reload-policy` and `audit` only from the owner), the deadline, the owner's policy and an audit line written before the answer; a request whose audit line cannot be written is not done (MC-10.6). The machine's authorities are a trait (`Machine`) the service will bind to `init`'s grants.
  - **Done:** `status`; `service-control`; files and the log ring out in pieces of 32 KiB with their SHA-256, a file that changes after it was hashed refused; files in, in order, no more than announced, checked by size and SHA-256 before they are published, discarded otherwise, an existing file kept unless replacing was asked; `cancel`; the owner's `reload-policy` (a file that does not parse leaves no policy) and `audit`.
  - **Answered `unsupported` when the policy allows them:** packages (0008), updates (0009), reboot (0010), the screen, test scripts (501).
- **Tests:** `tests/effector_gateway_host.rs`, 9 host tests against a fake machine, in `scripts/host_tests.sh`.
- **Still open:** the service binary (`decode`, the generated replies, `Machine` over `init`'s grants), which needs 750-KRN-0119's boot image, badges and slots, 0120's writable scope and 0121's lifecycle client; the session's use of it in place of its own denial; the `effector` suite's gateway checks and the `isolation` row.
## Progress — 2026-10-22

- **The service binary, `effector_gw/src/main.rs`.** It serves `idl/effector.wit` through `decode` and the generated replies. Its `Machine` acts over `init`'s grants:
  - `sysinfo` for memory;
  - `init`'s lifecycle client for the services and `start`/`stop`/`restart`;
  - the log ring, and the newest `log:bootNNNN.log`;
  - a reading client for the policy and the file roots;
  - the writable scope for deliveries, written to `<path>.part` and renamed;
  - its private directory for the audit file.
- **How it starts.** It serves only when `init` starts it with `--service`; from the shell it says so and ends (the `effector` QEMU suite checks that).
- **A missing grant fails its requests.** Without the private directory, no audit line can be written, so every request is refused (MC-10.6).
- **The badges and slots** are one table in `libmind/src/effector.rs` (`mind::effector`), for `init` (750-KRN-0119) and the shell (750-APP-0093). A client with any other badge gets nothing (`Badge::Other`, a tenth host test).
- **Built** for x86_64 and aarch64 (`02_build.sh`). It is not run as a service yet: that needs 750-KRN-0119's boot image and arms, 0120's writable scope, 0121's lifecycle client, and `vfs_server`'s badges for `system/effector_gw`.
- **Still open:**
  - the session handing commands to it, instead of denying them;
  - the QEMU check of each request in a mode.
- **Later the same day: the first action through the gateway, `collect_file`.**
  - The session asks `effector_gw` to open the path and uploads the bytes to `/api/commands/file` with their SHA-256.
  - The answer is `completed`, or `failed` with the gateway's reason. It is journaled before its ACK and runs once only.
  - Host tests in `effector_session` (22), `effector_journal` (4) and `effector_wire` (9).
  - The `effector` QEMU suite on x86_64: a 50 000-byte file under the policy's `collect` root is uploaded and answered `completed`. One outside it is answered `failed: denied` and not sent. Both are audited.
  - KRN's 0153 (screen), 0121 (lifecycle) and 0120 (delivery scope) are made on their branch (d69f0ad6). The gateway's writer then names paths relative to the scope.
