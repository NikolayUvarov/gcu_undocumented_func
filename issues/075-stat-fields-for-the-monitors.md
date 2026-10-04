# 075 — `STAT` fields the monitors had on the tools branch

**Type:** kernel · **Owner:** kernel track (`kernel/src/scheduler/stat.rs`, `kernel/src/pci.rs`, `kernel/src/paging.rs`, `common/abi.rs`, the shell's `stat` commands) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** track G, T1 · **Constitution:** MC-10.2

## Problem

The merge of the tools branch ([051](../issues-done/051-merge-main-into-tools.done)) moved `sysmon`, `top`, `memmap`, `hw` and the shell's observation commands onto `main`'s `STAT` records (issue [035](../issues-done/035-observe-and-stat.done)). The tools branch's own `STAT` (record [059](../issues-done/059-observation-abi.done)) reported more, and the tools show less now:

- `StatMemory`: the largest free block of the kernel arena (fragmentation: whether a large image still fits), page tables, mapped foreign memory, and the kernel's table limits (the monitors now carry `TASKS_LIMIT` 32 and `ENDPOINTS_LIMIT` 127 as constants);
- `StatTask`: kernel memory per task (context, mailbox, info and exit pages, page tables) and whether the task has the focus (`sysmon` asks `TASK_LIST` for it);
- `STAT_PHYSMAP`: the kernel image and the DMA regions in the platform layout;
- `STAT_VMAP`: the guard page below the stack — `VMAP` walks only present pages, and the guard is unmapped on purpose, so `memmap` no longer shows it;
- `StatDevice`: the PCI location (bus, device, function) and which BARs are I/O ports — the kernel's PCI record has them (`pci::Device`), but they are private and not copied out, so `hw` lists devices by index;
- `StatCap`: the endpoint index of an endpoint capability (to match `caps` with `endpoints`);
- `StatEndpoint`: the server, the number of holders, whether an IRQ is bound;
- `StatIrq`, `StatDevice`: the holder is the first task in the table that holds the capability. That is `init`, which keeps a copy of every grant to restart a driver, rather than the driver that uses it: `hw` shows `init` as the holder of IRQ 1 instead of `ps2_kbd`.

## Decisions

- None of the fields is authority, and none exposes task memory or physical addresses of task memory (MC-10.2). The kernel image is kernel memory and goes into the platform layout. **DMA regions do not**: they are mapped into a driver, so their physical addresses are physical addresses of task memory; `StatMemory.dma` keeps reporting their total size.
- **Holder:** the task that holds the most recently derived copy of the capability (the highest derivation node; node identities are never reused). A driver's copy is derived from the one `init` keeps, so the driver is reported; `init` is reported only while nobody else holds a copy. The same rule gives the server of an endpoint (among the holders of the read right). A count of holders is reported next to it.
- Records keep their layout and grow at the end; reserved fields that become used keep their place. `STAT_VERSION` becomes 2 (`libmind::stat::read` rejects another version, and the boot image is always rebuilt as a whole); readers accept records larger than they know.

## Plan

1. `common/abi.rs`, `STAT_VERSION` 2:
   - `StatMemory` + `largest_free` (found by trial allocations, only when `msg[1]` = 1 asks for it: it costs time under the scheduler lock), `page_tables`, `shared` (memory of other owners mapped by tasks), `tasks_limit`, `endpoints_limit`;
   - `StatTask`: `reserved` → `focus` (u8) and a reserved byte; + `kernel_bytes` (context, mailbox, info and exit pages, page tables);
   - `STAT_PHYSMAP`: kind `PHYS_KERNEL` (the kernel image; `__kernel_end` in `kernel/linker.ld`);
   - `STAT_VMAP`: kind `REGION_GUARD` (no rights) for the page below the stack;
   - `StatCap`: `reserved` → `endpoint` (the endpoint index of an endpoint capability; a label, as in `StatEndpoint`);
   - `StatEndpoint` + `server` (PID), `holders`, `irq` (the line bound to it, 0 if none);
   - `StatIrq`: `reserved` → `holders`; `holder` by the rule above;
   - `StatDevice` + `location` (bus << 8 | device << 3 | function), `io_bars` (bit *i*: BAR *i* is an I/O port range); `holder` by the rule above.
2. `kernel/src/scheduler/stat.rs` fills them; `kernel/src/pci.rs` exposes the location, `kernel/src/paging.rs` the page-table count.
3. `libmind::stat` names the new kinds; the shell's `stat <class>`, `free`, `irqs`, `devices`, `endpoints`, `caps` and `stat <pid>` print the new fields.
4. The consumers in the tools (`idl/sysinfo.wit`, `sysmon`, the monitors) follow in [076](076-monitors-show-restored-stat-fields.md). `sysmon` already passes `holder` through, so `hw` shows the driver again with this issue alone.

## Acceptance criteria

- QEMU `services` suite: `stat memory` reports a largest free block no larger than the free memory, page tables, and the limits 32 and 127; `stat tasks` gives every task kernel memory and marks the shell's focus; `physmap` shows the kernel image; `stat vmap` of the shell shows the guard page below the stack; `devices` shows `00:01.1` for the IDE controller; `irqs` names `ps2_kbd` as the holder of IRQ 1 (with `init`'s copy counted); `endpoints` name each service as the server of its endpoint; `caps` of a client shows the endpoint index.
- QEMU `tools` suite: `hw` names `ps2_kbd` as the holder of IRQ 1 again.
- All QEMU suites, the one-CPU subset and the host tests pass; CI is green.

## Related

[051](../issues-done/051-merge-main-into-tools.done), [076](076-monitors-show-restored-stat-fields.md), [docs/tools/README.md](../docs/tools/README.md) §4.4–4.7.
