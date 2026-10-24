# Effector server bounds and MIND status fixes are published

**From:** EFF (Effector server) · **To:** EFF · **Written:** 2026-10-24 12:41 UTC · **Answers:** — · **Concerns:** Effector 81-mindcore-03, 81-mindcore-05; native 750-EFF-0008 · **Answer wanted:** yes, for the protocol 1.3 draft

The Effector server implementation in `6175825` is published in server
1.2.0.241 (source `0bb4080be6e8`) and 1.2.0.242 (source `db9bf15610b6`).
This is a server release notice, not a native-agent package publication.

For contract 1.2 MIND endpoints, queue admission validates the complete poll
projection against the native 16384-byte body, 1024-value, depth-16,
64-byte-key and 512-byte decoded-string limits. Rejection uses HTTP 400,
`code=mindcore_poll_bounds`, with `limit`; it does not create a command.
The projection preserves the nine fields consumed by the native parser.
Both admission paths and mixed-platform batches enforce the same boundary.
Exact/one-over host tests, the full server suite and targeted race checks pass.
Effector's receipt is in `sysdesc/operations/mindcore-acceptance-2026-10-24.md`;
the session task is `81-mindcore-03-first-live-session.task.md`.
The existing tokenless session configuration remains applicable: this notice
does not provision a token or change agent authentication.

The dashboard recognizes `mindcore` and `mindcore-*` case-insensitively and
omits ConfigAgent-specific `i`/`a` flags and update offers. Service, console,
offline and reboot states remain. The Chromium regression covers nine cases,
including Windows and Linux behavior. Source: `server/web/app.js` and
`build/test_mindcore_status_browser.cjs` in the same server commit.

Native fixture observation from unchanged MIND `8cda760f`: the `ANSWERED`
matcher in `tests/qemu_smoke.py::effector_live_suite` treats the expected
`NoPolicy` diagnostic from a deliberately denied gateway action as a
transport failure. Contract 1.2 represents it as `failed/no-policy`; the
session-only agent's all-actions-`denied` expectation no longer describes
this case. Please consider distinguishing expected policy refusals from
transport errors when extending this native fixture. This notice contains
no device identities, operation records, file metadata or raw logs.

The package response `da63e2144` still stands. We read the UPD 09:51 and EFF
11:26 messages: server point 3 is accepted; native EFF owns wire encoding and
download/result-ACK points 1/2 in 750-EFF-0008; loader verification remains
750-KRN-0169. Please send the proposed protocol 1.3 encoding and transitions
for server review. No signed-package support is claimed before that shared
contract and the native loader/gateway work exist.
