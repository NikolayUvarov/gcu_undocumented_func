# 050 — `logd`, `dmesg`, `svc` (tools F10, T3)

**Type:** tool · **Owner:** tools track · **Priority:** P3 · **Status:** open · **Roadmap:** track G, T3 · **Blocked by:** 044

## Problem

Logs live in per-task rings drained by `logs`; there is no service view or control besides `ps`, `kill` and `RUN`.

## Plan

- `logd`: bounded ring with sender stamping and gap counting; `dmesg`. `svc`: list services with state, PID, restarts and quarantine from `init` (C6), restart on request.

## Acceptance criteria

- Suites: records carry the stamped sender; a service killed from `svc` is restarted by `init`; a quarantined one is shown as such.

## Related

[docs/tools/README.md](../docs/tools/README.md) (plan of the tools track, branch `claude/wizardly-franklin-kec1a9`) F10.
