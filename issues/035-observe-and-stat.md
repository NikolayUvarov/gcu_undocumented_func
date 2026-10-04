# 035 — OBSERVE privilege, STAT system call, firmware memory map (tools F5)

**Type:** kernel · **Owner:** kernel track (coordinates `kernel/src/scheduler.rs`, `common/abi.rs`) · **Priority:** P1 · **Status:** open · **Roadmap:** track G, T1 · **Constitution:** MC-10.2, MC-5.4

## Problem

Monitoring tools (`top`, `memmap`, `load`) need statistics the kernel keeps only partially (10 ms tick counts) and can read them today only with the process-control privilege, which also allows kill, focus and reading other tasks' output.

## Plan

- Privilege `CAP_KIND_OBSERVE`: `TASK_LIST`, `CPU_INFO`, `KERNEL_HEAP`, `FAULTS` and `STAT` accept OBSERVE or CONTROL; kill, focus, halt, logs, console stay CONTROL only.
- `STAT(class, buffer, capacity, argument)` → versioned fixed-size records with a `StatHeader`; copies bounded by the table sizes. Classes: TASKS, CPUS, MEMORY, PHYSMAP, VMAP(pid), CAPS(pid), ENDPOINTS, IRQS, DEVICES (fields as in the tools plan, F5).
- Accounting: TSC time per context switch (run ns per task, busy/idle ns per CPU), counters per IRQ line and endpoint, kernel arena by category.
- Bootloader: the UEFI memory map in `BootInfo`.
- Never exported: memory contents, physical addresses of task memory, anything usable as authority.

## Acceptance criteria

- `isolation` case: OBSERVE cannot kill, focus or read logs; STAT rejects a short buffer.
- QEMU suite: task count of STAT = `ps`; arena used = `heap`; the VMAP of a test application matches its known layout.

## Related

[docs/tools/README.md](../docs/tools/README.md) (plan of the tools track, branch `claude/wizardly-franklin-kec1a9`) F5; [045](045-sysmon-and-monitors.md).
