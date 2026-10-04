# 039 — Observation ABI: OBSERVE privilege, `STAT`, firmware memory map

**Type:** architecture · **Priority:** P0 · **Status:** open · **Blocked by:** — · **Roadmap:** track G, tools plan F5 · **Constitution:** MC-1.1, MC-3.3, MC-5.4, MC-10.2

## Problem

All observation needs the process-control privilege (it also allows kill, focus, halt and reading logs) and gives too little: no per-task memory, no idle time, no IRQ/IPC counters, no address-space map, no firmware memory map (the bootloader discards it).

## Plan

- Privilege `CAP_KIND_OBSERVE`: `TASK_LIST`, `CPU_INFO`, `KERNEL_HEAP`, `FAULTS` and `STAT` accept OBSERVE or CONTROL; kill, focus, halt, logs and console stay CONTROL.
- `STAT(class, buffer, capacity, argument)` → versioned fixed-size records after a `StatHeader`; copies are bounded by the table sizes. Classes `TASKS`, `CPUS`, `MEMORY`, `PHYSMAP`, `VMAP(pid)`, `CAPS(pid)`, `ENDPOINTS`, `IRQS`, `DEVICES` (fields in the tools plan, F5).
- Accounting: TSC at every context switch (run ns per task, busy ns per CPU), counters per IRQ line and per endpoint, kernel-arena bytes by category.
- Never exported: memory contents, physical addresses of task memory, anything usable as authority.
- Bootloader: the UEFI memory map returned by `exit_boot_services` is copied into the handoff pages; `BootInfo` gets `memory_map` and `memory_map_len`.

## Acceptance criteria

- The isolation fixture shows that OBSERVE cannot kill, focus, halt or read logs and that STAT rejects bad buffers.
- QEMU checks in the `tools` suite: task count in `STAT` = `ps`; the address-space map of a test application matches its known layout; busy time grows on a CPU running `busy_app`.
- Profile updated: kernel-objects.md, conformance row MC-10.2.

## Related

[docs/tools](../docs/tools/README.md) F5.
