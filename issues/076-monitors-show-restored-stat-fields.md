# 076 — The monitors show the restored `STAT` fields

**Type:** tool · **Owner:** tools track (`idl/sysinfo.wit`, `sysmon/`, `monitor/`) · **Priority:** P2 · **Status:** open · **Blocked by:** — ([075](../issues-done/075-stat-fields-for-the-monitors.done) done) · **Roadmap:** track G, T1 · **Constitution:** MC-10.2

## Problem

Issue [075](../issues-done/075-stat-fields-for-the-monitors.done) adds back to `STAT` what the monitors lost in the merge ([051](../issues-done/051-merge-main-into-tools.done)). `sysmon` and `idl/sysinfo.wit` do not carry those fields yet, and the monitors still show less than on the tools branch: `hw` lists PCI devices by index, `memmap` has no largest free block, names the guard page "?", and takes the table limits from constants, `top` has no kernel memory per task. The QEMU `tools` suite was relaxed in the merge to match.

## Plan

- `idl/sysinfo.wit` **2.0** (new fields change the layout of existing records, a major version): `memory` + `largest-free`, `page-tables`, `shared`, `tasks-limit`, `endpoints-limit` (`memory` asks the kernel for the largest free block); `task` + `kernel`; `capability` + `endpoint`; `endpoint-info` + `server`, `holders`, `irq`; `irq` + `holders`; `device` + `location`, `io-bars`. Regenerate the bindings.
- `sysmon` copies the fields; the focus flag comes from `StatTask.focus` instead of `TASK_LIST`.
- `monitor`: `memmap` shows the largest free block, page tables and mapped shared memory in the kernel-arena view, the limits from `memory` (no `TASKS_LIMIT`/`ENDPOINTS_LIMIT` constants), "guard" and "kernel image" as kinds; `hw` shows `bus:device.function` and marks I/O BARs; `top` shows a task's kernel memory and, in the details, the endpoint index of each endpoint capability; `load` scales the task graph by the limit from `memory`.
- Host tests `tests/sysmon_host.rs`, `tests/monitor_host.rs` cover the new fields.

## Acceptance criteria

- The QEMU `tools` suite checks again what the merge relaxed: `hw` shows `00:01.1  010180  IDE controller`; `memmap` shows `0x0000008001000000  4.0K  ---  guard` in clock's address space and the largest free block in the arena view; the task limit comes from the kernel.
- All QEMU suites and host tests pass; CI is green.

## Related

[075](../issues-done/075-stat-fields-for-the-monitors.done), [061](../issues-done/061-top-memmap-load-hw.done), [060](../issues-done/060-sysmon.done), [docs/tools/README.md](../docs/tools/README.md) §4.4–4.7.
