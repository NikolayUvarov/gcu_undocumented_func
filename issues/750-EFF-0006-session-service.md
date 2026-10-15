# 750-EFF-0006 — `effector`: the session with the Effector server

**Type:** service · **Owner:** `EFF` track (open) · **Priority:** P1 · **Status:** open · **Blocked by:** 0001–0004; [750-NET-0019](../issues-done/750-NET-0019-http-for-services.done) (an HTTP/1.1 client for a service), [750-NET-0020](../issues-done/750-NET-0020-tls-with-two-pins.done) (a pinned leaf certificate), [750-NET-0021](../issues-done/750-NET-0021-json-in-the-parser-service.done) (JSON in `parse`); [750-NET-0014](../issues-done/750-NET-0014-the-bounds-of-a-flow.done) (a flow for a long-running service); [requests-KRN.md](requests-KRN.md) (boot images and `init`'s grants); [requests-APP.md](requests-APP.md) (the shell's `effector` command, for the configuration) · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-11.3, MC-11.4, MC-11.6, MC-11.11, MC-7.1, MC-7.5, MC-7.6, MC-6.6, MC-6.9

## Problem

Something on the machine must keep the connection to the server, report the machine and take commands. It must do that without holding any authority over the machine itself (MC-11.11).

## Plan

- **A boot service `effector`, off by default (173).** Its grants from `init`:
  - one flow to the configured server;
  - a `tls` client;
  - a `parse` client;
  - `rtc`;
  - a read-only `vfs` view of `data/effector/`;
  - its own private directory in `system/`, for the token;
  - an endpoint to `effector_gw`.
- **Configuration** in `data/effector/` (written by the owner through the shell):
  - the server's HTTPS URL;
  - the current and next SHA-256 leaf-certificate pins for `tls.wit` 1.2 `connect-pins` (750-NET-0020), as [contract 1.0](../docs/effector/protocol.md) requires; verify the pins through the owner's trust channel in `sst-test-deploy: issues/81-mindcore-01-agent-and-live-acceptance.task.md`, without assuming a server-side publication feature exists;
  - the token, taken into the private directory on first start, with the shell's copy removed;
  - the agent ID, made once from `mind::random` and kept.
- **Trust both ways.** The server's pinned leaf certificate; the machine's token, and the device key's certificate (`tls` offers it), which identifies the machine once Effector accepts agents by device key.
- **Only HTTPS with a pinned certificate.** There is no plain-HTTP mode and no fallback. A wrong pin, a name mismatch or a redirect stops the connection and is logged.
- **Heartbeat** every one to two seconds over one kept-alive connection:
  - identity;
  - `os_version: "mindcore"`, `arch` `x64` or `arm64`, release version;
  - the device key's fingerprint and the image's manifest hash (350);
  - the boot services from `effector_gw`'s `status`;
  - the update state;
  - `system_state`;
  - time from `rtc`.
- **Commands:**
  - a second connection holds the server's wake-up stream;
  - polling runs after every wake-up and at least every ten seconds;
  - the service parses only the framing; each head and JSON body goes to `parse` (MC-11.11), and the typed result is checked against what was expected (MC-11.5);
  - each command becomes a typed request (0003), and an action without one is answered `denied`.
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
- The service holds only the grants listed above (the `isolation` suite's style of check), and [docs/network/airlock.md](../docs/network/airlock.md) gains its row.

## Related

[750-EFF-0001](750-EFF-0001-protocol-contract.md), [750-EFF-0002](750-EFF-0002-test-server.md), [750-EFF-0007](750-EFF-0007-gateway-service.md), [351-NET-0002](../issues-done/351-NET-0002-https-for-programs.done), 173-KRN-0035 (`init` reads the service configuration; on the kernel track's branch until it merges).

## Implementation inputs — 2026-10-15

[Contract 1.0](../docs/effector/protocol.md) and the [host fixture](../docs/effector/README.md) now exist. The runtime service has not been implemented. HTTP POST/headers/keep-alive, two certificate pins and parser-service JSON are already present in the linked NET tasks; use them rather than recreating transports.

The first token-authenticated slice can heartbeat and poll independently while all local actions are denied. It needs a native AgentToken; the owner's operator API key is not a substitute. The device-certificate part above remains separate: `connect-pins` currently cannot request a client certificate, and the reviewed server does not yet persist the proposed device-key/manifest heartbeat fields. Keep those acceptance/evidence requirements open until an explicit protocol/interface extension is implemented.
## Network latency limit found by the hardware audit (2026-10-15)

`550-NET-0031` bounds each card turn, not the whole network service turn.
Three cards with `CARD_MS=2000` can consume six seconds of driver waits in one
`Stack::poll`, before CPU and scheduling overhead. The main loop polls both
before receiving clients and after handling one. The reviewed Effector server
marks an endpoint offline after five seconds, so independent heartbeat work
alone does not establish stable online status while unrelated cards stall.

Before closing session-service acceptance, inject two failing peers beside a
healthy link and measure heartbeat gaps and socket-service latency. Use that
evidence to implement a shared turn budget, fair rotation or asynchronous
driver calls. Do not hide the gap by broadening server timeouts or transfer
per-card boundedness into a heartbeat guarantee.
