# 700-EFF-0007 — `effector_gw`: the gateway that acts on the machine

**Type:** service · **Owner:** `EFF` track (open) · **Priority:** P1 · **Status:** open · **Blocked by:** [700-EFF-0003](700-EFF-0003-interface-and-policy.md); [requests-KRN.md](requests-KRN.md) (boot images and `init`'s grants, a writable scoped `vfs` client, a lifecycle client limited to named services) · **Main task:** [700](700-effector-agent.md) · **Constitution:** MC-3.1, MC-3.4, MC-3.7, MC-3.8, MC-10.2, MC-10.3, MC-10.7, MC-6.8, MC-6.9

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

[700-EFF-0003](700-EFF-0003-interface-and-policy.md), [700-EFF-0006](700-EFF-0006-session-service.md), `idl/init.wit`, `idl/log.wit`, `idl/sysinfo.wit`, `idl/vfs.wit`.
