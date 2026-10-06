# 171-APP-0006 — `top`, `memmap` and `load`: the bounds the kernel has now

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [171](../issues-done/171-limits-from-the-hardware.done) · **Roadmap:** G (monitors) · **Constitution:** MC-10.2, MC-12.3

Requested by the kernel track in `requests-APP.md` (2026-10-06): "the capability table grows" and "the task and endpoint limits are 65 535 now".

## Problem

Issue 171 changed the bounds the monitors show:

- A task's capability table starts with 96 slots (`CAP_SLOTS`) and grows on demand up to 4095 (`CAP_SLOTS_MAX`, 171-KRN-0004). `top`'s details and `memmap`'s quota view still show a task's capabilities as `n/95`. The shell's `stat <pid>` shows `CAPS=n/4095`.
- There is no fixed count of tasks or endpoints (171-KRN-0002). STAT's `tasks_limit` and `endpoints_limit` report the root quota, 65 535 (`QUOTA_MAX`), which is what the 16-bit fields of `SPAWN` can delegate, not what the machine can run. `memmap` and `top` show `Tasks n/65535`. The load monitor scales its task graph to `tasks_limit`, so the graph lies flat at the bottom.

## Plan

- Capabilities as `n/4095` (`CAP_SLOTS_MAX - 1`).
- `memmap` and `top`: the counts of tasks and endpoints alone. The root quota stays visible where it is a quota: `memmap`'s quota view and the shell's `quotas`.
- `load`: the task graph scaled to its own maximum, like the counters; its title gives the count and the maximum.

## Acceptance criteria

- `top` and `memmap` show `n/4095` capabilities and the counts of tasks and endpoints without 65535.
- The task graph shows a change from 20 to 60 tasks (host test).
- The QEMU monitor checks and the host tests check the new texts.

## Related

[171-KRN-0002](../issues-done/171-KRN-0002-task-and-endpoint-tables.done), [171-KRN-0004](../issues-done/171-KRN-0004-growing-capability-tables.done), [u012](../issues-done/u012-load-graphs-aligned.done).
