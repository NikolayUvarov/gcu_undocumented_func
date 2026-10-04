# 080 — `ipc`: endpoints, holders and the wait-for graph

**Type:** tool · **Owner:** tools track · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Roadmap:** track G, T3 · **Constitution:** MC-10.2

## Problem

The plan's `ipc` tool ([docs/tools](../docs/tools/README.md) §2.2, §4.7) does not exist; the shell's `endpoints` command prints raw records. `STAT` version 2 (075) has what it needs: each endpoint's server, holders, waiting senders and receivers, bound IRQ, message/busy/timeout counts, and each task's wait state.

## Plan

- `idl/sysinfo.wit` 2.1: `holders(index: u32) -> result<list<holder, 64>, error>` — the tasks holding a capability for that endpoint (PID, slot, rights, badge), collected by `sysmon` from `STAT_CAPS`.
- `ipc` in `monitor/`: a table of endpoints (index, server, holders, waiting senders/receivers out of `ENDPOINT_QUEUE`, messages, busy, timeouts, IRQ), sortable; Enter shows the holders of one endpoint; a second view lists who waits for whom (task → endpoint → server, task → server for a reply) with cycles marked as deadlocks; `REQUEST_SYSINFO`.
- Host test in `tests/monitor_host.rs` (a fake with a cycle); QEMU `tools` suite: `ipc` shows the services' endpoints with their servers and, with `pong` waiting on `ping`, the wait edge.

## Acceptance criteria

- The tests above pass; README and `docs/tools` describe `ipc`.

## Related

[075](../issues-done/075-stat-fields-for-the-monitors.done), [076](../issues-done/076-monitors-show-restored-stat-fields.done), [061](../issues-done/061-top-memmap-load-hw.done).
