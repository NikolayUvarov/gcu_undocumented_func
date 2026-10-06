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

- `cpus(start)` returns a page of 64 CPUs from `start` on (the kernel starts up to 255 on x86, 256 on aarch64; 256 records in one reply would take 14 KiB of a program's stack once decoded).
- `caps(pid, start)` returns a page of 64 from `start` on; `top`'s details ask page after page.
- A sample keeps the busy share of CPUs 0–15 one by one (`busy`, as many as are online) and of every CPU as the mean (`busy-total`) and the busiest (`busy-max`). Sixteen graphs is what one screen holds beside the counters; keeping every CPU in every sample would make a reply of 150 samples 80 KiB, more than a program's 64 KiB stack holds once decoded.
- `sysmon` reads every CPU (`mind::stat::each`) and keeps the counters of each.
- `top` takes each CPU's share from its busy and idle time between two refreshes (`cpus`), for every CPU: two bars to a row, more for more CPUs while a bar keeps 24 cells, at most 8 rows, and a last row with the mean and the busiest of the CPUs that do not fit.
- `load` draws a graph for each of the 16 a sample keeps and, with more CPUs, the mean and the busiest of all.
- `uptime` reports the mean of every CPU.

## Acceptance criteria

- With 16 CPUs, `top` shows a bar and `load` a graph for each of the 16 (QEMU, groups "16 CPUs").
- `top`'s details list every capability of init, more than 64 (QEMU, `tools` suite).
- Host tests: 16 and 64 CPUs in `top`, 20 in `load`; shares since the last refresh.

## Related

[171-APP-0002](../issues-done/171-APP-0002-sysinfo-pages.done), [171-KRN-0003](../issues-done/171-KRN-0003-every-cpu.done), [171-KRN-0004](../issues-done/171-KRN-0004-growing-capability-tables.done), [081](../issues-done/081-caps-tool.done).
