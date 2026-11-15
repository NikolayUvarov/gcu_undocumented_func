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

- [x] **The `effector-update` suite, x86 and aarch64,** with 801-KRN-0242's `init`: the trial is confirmed on the agent's `accepted`, and the command is answered `updated 6, slot B confirmed`. LEG9, 2026-11-15, `d48f419c`: PASS on both.
- [x] **`init`'s log** shows the steps, with no flood while the agent retries. LEG9, 2026-11-15, `d48f419c`, on both: the last step before `init`'s confirmation is `accepted`, at most 8 step lines.
- [ ] **S8 on the bench** (801): one release built to lose the network is refused on the agent's last step, and falls back.

## Progress (2026-11-15): `init`'s confirmation checked in the suite

- **KRN's request** of 2026-11-15 (recorded in `issues/requests-EFF.md` for 801-KRN-0242's criterion 2) is taken here, as criteria 1 and 2 of this task ask the same.
- **The `effector-update` suite** now reads `dmesg -s init` once the agent has answered the command `completed`, and checks:
  - `[INIT] TRIAL BOOT CONFIRMED: THE EFFECTOR SERVER ACCEPTED THE AGENT'S HEARTBEAT` is there, and neither of `init`'s other confirmations (every boot service started; the modes turned off);
  - the last `[INIT] THE EFFECTOR AGENT: <step>` before it is `accepted`, and there are at most 8 such lines.
- **Why then, not at the kernel's `THE TRIAL BOOT IS CONFIRMED`:** that line is the kernel's own deadline lifted, about 1 s into the boot, while `init` still holds the trial for the agent's word. The first run read `init`'s log there and found only `started`. The updater confirms only on `init`'s word (801-UPD-0037), so once the agent answers `completed`, `init` has confirmed.

## Related

[801](801-the-bench-stays-manageable.md), [801-EFF-0029](801-EFF-0029-the-fallbacks-report-on-the-first-connection.md), [751-EFF-0023](751-EFF-0023-the-agent-installs-a-developers-release.md).
