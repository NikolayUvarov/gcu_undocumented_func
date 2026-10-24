# 750-EFF-0015 — The Effector agent's mark on the screen: red, yellow or grey

**Type:** gateway, session, IDL · **Owner:** `EFF` · **Priority:** P1 · **Status:** open · **Blocked by:** — (800-KRN-0156, `display.wit` 1.2 `remote`, is in `main`) · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-3.8, MC-8.1, MC-10.2, MC-10.7

## Problem

- **The maintainer's word (2026-10-23).** The screen must show who can reach the machine remotely and what they may do there.
- **What LNK asked** (`EFF/2026-10-23T0254Z-LNK-the-agents-mark-on-the-screen.md`, 800-LNK-0020): the network agent shows a filled circle beside the link's ring.
  - Red: it can change the machine.
  - Yellow: it can only read.
  - Grey: it is not connected.
- **What the kernel made** (800-KRN-0156): the compositor draws it while `display.remote(state)` is called each second, and drops it 3 s after the last call.

## Plan

1. **Only the session knows the connection.** It reports to the gateway every heartbeat, in `effector.wit` 1.3 `connection`: connected, and activated by the server.
2. **The gateway calls `display.remote` each second.** It holds slot 15, and the policy is its own.
   - **Grey:** the last report says not connected, or not activated.
   - **Red:** connected and active, and the policy names anything that changes the machine (`deliver`, `service`, `packages`, `update` beyond `check`, `reboot`, `test`).
   - **Yellow:** connected and active, and the policy names only reading (`screen`, `log`, `collect`).
   - No report for 3 s: no call, so the mark goes when the session stops or hangs.
3. **Delivering into an inbox counts as a change** (red): it writes files the machine then has.

## Acceptance criteria

- [x] Host tests: the colour from the policy and the reports; grey after 3 s without one (`tests/effector_gateway_host.rs`).
- [ ] The `effector` QEMU suite on x86_64 and aarch64: `[COMPOSITOR] REMOTE MARK Effector: Changing` once connected under a policy that delivers, `Unconnected` with the rogue server. Passed on x86_64 (2026-10-24).
- [ ] On the MacBook: red with today's policy.

## Related

800-LNK-0020, 800-KRN-0156, [750-EFF-0007](750-EFF-0007-gateway-service.md), [750-EFF-0012](750-EFF-0012-run-against-the-effector-server.md).

## Progress — 2026-10-24

- **What is made.**
  - `effector.wit` 1.3 `connection`: the session's report each heartbeat.
  - `effector_gw` calls `display.remote` each second between requests, waiting at most 1 s for one.
  - `Policy::may_change` decides red against yellow.
- **The maintainer asked for more** (KRN/…-EFF-a-request-the-remote-marks-blink-on-activity…):
  - the marks blink while their path is active;
  - they sit in the screen's bottom status line.
