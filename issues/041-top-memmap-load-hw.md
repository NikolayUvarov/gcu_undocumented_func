# 041 — `top`, `memmap`, `load`, `hw`

**Type:** feature · **Priority:** P0 · **Status:** open · **Blocked by:** 034, 040, 042 · **Roadmap:** track G, tools plan §4.4–4.7

## Problem

The shell's `ps`, `cpus`, `heap` are one-shot text dumps; there is no interactive task monitor, memory map, load graph or device list.

## Plan

- `top`: header (uptime, task states, per-CPU busy bars, load averages, kernel memory, IPC/s, faults), task table with %CPU from run-time deltas, sorting (P/M/N/T), service filter (S), tree (t), task details (Enter).
- `memmap`: physical map (firmware map + platform layout), kernel arena (use, fragments, categories, limits), process address space (pmap) with guards and shared mappings, quotas.
- `load`: per-CPU, interrupt, syscall, IPC and memory graphs over 30 s / 10 min; `uptime` as a console summary.
- `hw`: CPU (CPUID), clocks, framebuffer, PCI devices with holders, IRQ lines, DMA regions.

## Acceptance criteria

- QEMU suite `tools`: each tool starts, shows values that agree with `ps`/`heap`/`cpus`, reacts to keys, exits with Esc; screendumps checked.

## Related

[docs/tools](../docs/tools/README.md) §4.4–4.7.
