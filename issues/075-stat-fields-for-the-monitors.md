# 075 — `STAT` fields the monitors had on the tools branch

**Type:** kernel · **Owner:** kernel track (`kernel/src/scheduler/stat.rs`, `common/abi.rs`) · **Priority:** P3 · **Status:** open · **Blocked by:** — · **Roadmap:** track G, T1 · **Constitution:** MC-10.2

## Problem

The merge of the tools branch ([051](../issues-done/051-merge-main-into-tools.done)) moved `sysmon`, `top`, `memmap`, `hw` and the shell's observation commands onto `main`'s `STAT` records (issue [035](../issues-done/035-observe-and-stat.done)). The tools branch's own `STAT` (record [059](../issues-done/059-observation-abi.done)) reported more, and the tools show less now:

- `StatMemory`: the largest free block of the kernel arena (fragmentation: whether a large image still fits), page tables, mapped foreign memory, and the kernel's table limits (the monitors now carry `TASKS_LIMIT` 32 and `ENDPOINTS_LIMIT` 127 as constants);
- `StatTask`: kernel memory per task (context, mailbox, info and exit pages, page tables) and whether the task has the focus (`sysmon` asks `TASK_LIST` for it);
- `STAT_PHYSMAP`: the kernel image and the DMA regions in the platform layout;
- `STAT_VMAP`: guard pages around the stack;
- `StatDevice`: the PCI location (bus, device, function) and which BARs are I/O ports;
- `StatCap`: the endpoint index of an endpoint capability (to match `caps` with `endpoints`);
- `StatEndpoint`: the server, the number of holders, whether an IRQ is bound.
- `StatIrq`, `StatDevice`: the holder is the first task that holds the capability, which is `init` (it keeps what it granted for restarts) rather than the driver that uses it; `hw` shows `init` as the holder of IRQ 1.

## Plan

- Decide which of these belong in `STAT` (none is authority; none exposes task memory or physical addresses of task memory).
- Add them at the end of the records with `STAT_VERSION` 2; readers already check `record_size`.
- `sysmon` (`idl/sysinfo.wit`, minor version) and the monitors show them again.

## Acceptance criteria

- `memmap` shows the largest free block, `hw` the PCI locations, `top` the kernel memory of a task; the QEMU `services` and `tools` suites check them.

## Related

[051](../issues-done/051-merge-main-into-tools.done), [docs/tools/README.md](../docs/tools/README.md) §4.4–4.7.
