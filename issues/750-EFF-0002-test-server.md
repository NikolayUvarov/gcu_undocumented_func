# 750-EFF-0002 — A test server on the host for the Effector contract

**Type:** test tool · **Owner:** `EFF` track (open) · **Priority:** P1 · **Status:** open · **Blocked by:** [750-EFF-0001](750-EFF-0001-protocol-contract.md) (the contract it serves) · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-12.1, MC-12.2

## Problem

The QEMU suites need a server on the host that speaks the contract of 0001. The real Effector server is a separate private project and cannot run in this repository's CI. Its behaviour is also what 0012 checks, not something the unit of work here should depend on.

## Plan

- **`tests/effector_server.py`:** Python's standard library only, in the manner of `scripts/serve_release.py`.
- **Transport:**
  - HTTPS with a test certificate made at run time;
  - a bearer token;
  - the agent routes of 0001.
- **A small control interface for the suite:**
  - queue an action with its payload;
  - read an operation's final state and result;
  - publish a package or a file;
  - read what the agent uploaded.
- **Fault hooks:**
  - cut the stream or a download in the middle;
  - delay or drop a poll response;
  - answer an ACK with 500 once;
  - present a certificate with another key;
  - redirect;
  - send unknown fields and an unknown action.
- **Bookkeeping:** the server records every request, so a suite can assert the order (no command run twice, every command ACKed).
- **Host tests** check the server itself against recorded exchanges from 0001.

## Acceptance criteria

- The server runs with `python3` on Linux, macOS and Windows (MSYS2), and is used by the effector suites in `tests/qemu_smoke.py`.
- Its host tests pass in `scripts/ci_local.sh`.
- The contract version it serves is the one in `docs/effector/protocol.md`.

## Related

[750-EFF-0001](750-EFF-0001-protocol-contract.md), [750-EFF-0006](750-EFF-0006-session-service.md), [750-EFF-0012](750-EFF-0012-run-against-the-effector-server.md), `scripts/serve_release.py`, `tests/qemu_smoke.py`.

## Progress — 2026-10-15

- Implemented `tests/effector_server.py`, a bounded host fixture for [contract 1.0](../docs/effector/protocol.md), with TLS 1.3, bearer authentication, `/effector` prefix, heartbeat, poll/SSE, string ACK, file/package exchange, host-only controls and the planned fault hooks. Python uses only its standard library; the existing OpenSSL executable generates temporary test certificates.
- `python3 tests/effector_server_test.py`: 12 wire-level host tests pass on Linux, including independent heartbeat during a delayed poll, a lost ACK, duplicate delivery, malformed framing/JSON, cut stream/download and a changed certificate. No live endpoint was registered.
- [Fixture guide and limits](../docs/effector/README.md). No QEMU effector suite exists until 0006/0007; its integration and macOS/MSYS2 checks remain open. The acceptance criteria above are unchanged.
