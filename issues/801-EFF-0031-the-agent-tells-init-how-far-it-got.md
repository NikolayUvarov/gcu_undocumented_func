# 801-EFF-0031 — The agent tells `init` how far it got, so a trial is confirmed on the server's word

**Type:** session · **Owner:** `EFF` · **Priority:** P1 · **Status:** in progress (2026-11-14) · **Blocked by:** — (built on KRN's 801-KRN-0242, which lands with it) · **Main task:** [801](801-the-bench-stays-manageable.md) (S1, S2) · **Constitution:** MC-9.9, MC-9.4

**Asked by:** KRN and UPD, `EFF/2026-11-14T0929Z-KRN-801-s1-the-agent-tells-init-its-step-init-wit-1-13.md`.

## Problem

- **Since 801-KRN-0242,** `init` holds a trial boot of a machine whose agent has a server, until the agent says `accepted`.
- **Without that word** the trial is refused 10 min after `READY`, and the machine falls back. That is safe, but no update would ever be kept.

## Plan

- **The calls.** `init.wit` 1.13's `agent-step`, on the client `init` gives the agent in `SLOT_LIFECYCLE` (`BADGE_NOTICE`). A step is said only when it changes:
  - `started` once the configuration is read;
  - `no-network`, with the error, when the flow grant is not attached or the server cannot be reached (`no-network`, `refused`, `timeout`, `denied`);
  - `no-tls`, with the error, when the handshake or the pin fails;
  - `refused`, with the HTTP status, when a heartbeat is answered but not accepted;
  - `accepted` on the first heartbeat the server accepts. That confirms the trial.
- **The update's own wait.** An update waiting for its confirmation after the restart (751-EFF-0023, 180 s) counts from the accepted heartbeat, not from the agent's start. A slow network before it then does not make a confirmed release look `not-confirmed`.

## Acceptance criteria

- [ ] **The `effector-update` suite, x86 and aarch64,** with 801-KRN-0242's `init`: the trial is confirmed on the agent's `accepted`, and the command is answered `updated 6, slot B confirmed`.
- [ ] **`init`'s log** shows the steps, with no flood while the agent retries.
- [ ] **S8 on the bench** (801): one release built to lose the network is refused on the agent's last step, and falls back.

## Related

[801](801-the-bench-stays-manageable.md), [801-EFF-0029](801-EFF-0029-the-fallbacks-report-on-the-first-connection.md), [751-EFF-0023](751-EFF-0023-the-agent-installs-a-developers-release.md).
