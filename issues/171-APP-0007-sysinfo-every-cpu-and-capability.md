# 171-APP-0007 — `sysinfo`: every CPU and every capability

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [171](../issues-done/171-limits-from-the-hardware.done) · **Roadmap:** G (monitors) · **Constitution:** MC-10.2

Requested by the kernel track in `requests-APP.md` (2026-10-06): "`sysinfo` and `sysmon` for any number of tasks, endpoints and CPUs". Tasks and endpoints are done in [171-APP-0002](../issues-done/171-APP-0002-sysinfo-pages.done); this is the rest.

## Problem

The kernel starts every CPU the firmware reports (171-KRN-0003: up to 255 on x86) and a task's capability table grows up to 4095 slots (171-KRN-0004). `idl/sysinfo.wit` 4.0 still caps:

- `cpus` at 8;
- `caps(pid)` at 64;
- a load sample's busy share at 8 CPUs (`busy-low`, `busy-high`).

So `top` and `load` show at most 8 CPUs, and `caps` and `top` at most 64 capabilities of a task.

## Plan

`sysinfo.wit` 4.0 has not reached `main` yet, so these changes go into it rather than into a 5.0:

- `cpus` returns every CPU, up to 256 a reply;
- `caps(pid, start)` returns a page from `start` on;
- a sample carries the busy share of every CPU (`busy`: a list), and `sysmon` keeps its history for every CPU.
- `top`, `load` and `caps` page through the lists and draw every CPU.

## Acceptance criteria

- With 16 CPUs, `top` and `load` show all 16 (QEMU, groups "16 CPUs").
- A task with more than 64 capabilities is listed whole by `caps` (host test).
- The host tests and the QEMU monitor checks pass.

## Related

[171-APP-0002](../issues-done/171-APP-0002-sysinfo-pages.done), [171-KRN-0003](../issues-done/171-KRN-0003-every-cpu.done), [171-KRN-0004](../issues-done/171-KRN-0004-growing-capability-tables.done), [081](../issues-done/081-caps-tool.done).
