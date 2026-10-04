# 050 — `svc` and lifecycle control from tools

**Type:** feature · **Priority:** P1 · **Status:** open · **Blocked by:** 038; full version after C6 · **Roadmap:** track G, C6 · **Constitution:** MC-6.1–6.9, MC-3.7

## Problem

Services are started and stopped only through shell commands; `top` cannot stop a task.

## Plan

- `idl/lifecycle.wit` served by `init`: list services (name, PID, state, restarts), start, stop, restart, stop an application. `init` holds the control privilege for this (it is the lifecycle owner, C6).
- The shell passes a lifecycle client to `svc` and `top`.
- Restart budgets, generations and failure notification remain C6.

## Acceptance criteria

- QEMU: `svc` restarts `rtc`; clients reach the new instance; `top` stops an application.

## Related

[docs/tools](../docs/tools/README.md) §2.3; [ROADMAP](../ROADMAP.md) C6.
