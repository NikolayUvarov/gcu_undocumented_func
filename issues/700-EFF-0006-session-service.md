# 700-EFF-0006 — `effector`: the session with the Effector server

**Type:** service · **Owner:** `EFF` track (open) · **Priority:** P1 · **Status:** open · **Blocked by:** 0001–0004; [requests-NET.md](requests-NET.md) (an HTTP/1.1 client for a service, a pinned leaf certificate, a flow for a long-running service); [requests-KRN.md](requests-KRN.md) (boot images and `init`'s grants); [requests-APP.md](requests-APP.md) (the shell's `effector` command, for the configuration) · **Main task:** [700](700-effector-agent.md) · **Constitution:** MC-11.3, MC-11.4, MC-11.6, MC-11.11, MC-7.1, MC-7.5, MC-7.6, MC-6.6, MC-6.9

## Problem

Something on the machine must keep the connection to the server, report the machine and take commands. It must do that without holding any authority over the machine itself (MC-11.11).

## Plan

- **A boot service `effector`, off by default (173).** Its grants from `init`:
  - one flow to the configured server;
  - a `tls` client;
  - `rtc`;
  - a read-only `vfs` view of `data/effector/`;
  - an endpoint to `effector_gw`.
- **Configuration** in `data/effector/` (written by the owner through the shell):
  - the server's HTTPS URL;
  - the current and next certificate pins (SHA-256 of the leaf certificate);
  - the token;
  - the agent ID, made once from `mind::random` and kept.
- **Only HTTPS with a pinned certificate.** There is no plain-HTTP mode and no fallback. A wrong pin, a name mismatch or a redirect stops the connection and is logged.
- **Heartbeat** every one to two seconds over one kept-alive connection:
  - identity;
  - `os_version: "mindcore"`, architecture, release version;
  - the boot services from `effector_gw`'s `status`;
  - the update state;
  - `system_state`;
  - time from `rtc`.
- **Commands:**
  - a second connection holds the server's wake-up stream;
  - polling runs after every wake-up and at least every ten seconds;
  - each command is decoded with `mind::json` into a typed request (0003), and an action without one is answered `denied`.
- **ACK.** The command ID and result go to a journal in `data/effector/` (through `effector_gw`, which holds the write) before the ACK. ACKs are retried with back-off until accepted or past the command's term, and a repeated ID returns the stored result (MC-6.6).
- **Pin rotation.** `update_agent_config` is accepted only over the verified connection and only within the current-plus-next set. The URL and the policy never change from the server (MC-7.5).
- **Losing the server.** The service reconnects with back-off and jitter, and nothing local waits for the server (MC-7.6).
- **Orderly stop.** One heartbeat with `system_state: "shutting_down"`.
- **Bounds** on bodies, header sizes, the journal and the number of commands in flight.

## Acceptance criteria

- **An `effector` suite in `tests/qemu_smoke.py`,** on x86_64 and aarch64, against the 0002 server:
  - the agent appears online and stays online for ten minutes;
  - it becomes ready for operations;
  - a shell action is answered `denied`;
  - a wrong pin is refused;
  - a cut stream and a cut connection recover;
  - a repeated command ID is not run twice;
  - an ACK answered with 500 is retried.
- The service holds only the grants listed above (the `isolation` suite's style of check).

## Related

[700-EFF-0001](700-EFF-0001-protocol-contract.md), [700-EFF-0002](700-EFF-0002-test-server.md), [700-EFF-0007](700-EFF-0007-gateway-service.md), [351-NET-0002](351-NET-0002-https-for-programs.md), 173-KRN-0035 (`init` reads the service configuration; on the kernel track's branch until it merges).
