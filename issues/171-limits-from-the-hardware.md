# 171 — Limits from the hardware: no fixed caps on tasks, endpoints, CPUs, capability slots or RAM

**Type:** kernel (main task) · **Owner:** kernel track (`KRN`) · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Roadmap:** K4 (accounted kernel objects: memory is still paid from the kernel heap), C1 · **Constitution:** MC-1.7, MC-3.13, MC-5.1

## Problem

Asked by the user (2026-10-06): MIND Core is meant to use the most of the machine it runs on, so its limits must come from the hardware alone. Today the kernel has fixed caps that no machine sets:

- 32 tasks for the whole system (`MAX_TASKS`), of which services take about 20;
- 8 applications (`MAX_APPS`, init's policy for loader's task quota);
- 128 IPC endpoints (`ENDPOINTS`), 4 for each application (`APP_ENDPOINTS`);
- 8 CPUs (`cpu::MAX`);
- 96 capability slots a task (`CAP_SLOTS`);
- 16 ranges of free RAM in the frame pool (`RANGES`);
- on x86-64, RAM below 4 GiB only: the kernel's identity map ends there (`IDENTITY_END`), so a machine with 8 GiB uses about 3.

MC-1.7, MC-3.13 and MC-5.1 ask that every kernel object be accounted and bounded by a quota delegated from above. They do not ask for fixed numbers. The root of the quotas can be the machine itself.

## Plan

1. **All RAM (x86-64).** The kernel maps the RAM the firmware lists above 4 GiB as well: 2 MiB pages, only the ranges that hold RAM, never device windows. The frame pool takes all of it. DMA regions stay below 4 GiB, since AC97, older AHCI and xHCI without 64-bit addressing cannot reach above. A QEMU suite with 6 GiB checks that memory above 4 GiB is used. The aarch64 identity map already covers 1 TiB.
2. **Tasks and endpoints from memory.**
   - The task table and the endpoint table grow as needed, without a compile-time size.
   - A task's kernel structures and an endpoint are paid from the spawner's or creator's memory quota (168 does this for images, stacks and screens).
   - init's root quota is the free memory less the recovery reserve (169). loader's application quota is all of it, so `MAX_APPS` and `APP_ENDPOINTS` go.
   - STAT, `sysmon`, `ps` and `top` list any number of tasks, page by page.
3. **CPUs.** As many as the firmware reports (MADT, PSCI): per-CPU state grows with them.
4. **Capability slots.** A task's capability space grows on demand within its memory quota; fixed slots stay as they are.
5. **The frame pool's ranges** grow with the firmware map.

Each step is its own commit, with the docs/profile statements (kernel-objects.md, bootstrap.md) changed with it.

## Acceptance criteria

- **QEMU x86-64, 6 GiB:** the frame pool reports the RAM above 4 GiB, and a program allocates there.
- **QEMU, both architectures:** more than 32 applications run at once (console programs; screens cost 4 MiB each). The limit comes only when memory runs out, as a clean refusal (`TASK LIMIT`/`NO MEMORY`) with the system left working, and the recovery reserve keeps the services restartable.
- **QEMU with 16 CPUs (x86, TCG):** all come online.
- **Host and QEMU tests:** a task with more than 96 capabilities, and the STAT pages.

## Related

[150](../issues-done/150-user-memory-beyond-the-arena.done), [168](../issues-done/168-task-memory-charged-to-spawner.done), [169](../issues-done/169-recovery-reserve.done), [024](../issues-done/024-accounted-kernel-objects.done).

## Progress

Split into `KRN` tasks (TRACKS.md), one for each step of the plan:

| Task | Step | Status |
|---|---|---|
| [171-KRN-0001](../issues-done/171-KRN-0001-ram-above-4g.done) | 1. All RAM on x86-64 | done (2026-10-06) |
| `171-KRN-0002` | 2. Task and endpoint tables from memory, paid from quotas; no `MAX_APPS`, `APP_ENDPOINTS`; STAT pages | next |
| [171-KRN-0003](../issues-done/171-KRN-0003-every-cpu.done) | 3. As many CPUs as the firmware reports; no tick for idle CPUs | done (2026-10-06) |
| [171-KRN-0004](../issues-done/171-KRN-0004-growing-capability-tables.done) | 4. Capability tables that grow on demand, up to 4095 slots (handle: 12 bits of slot, 20 of generation) | done (2026-10-06) |
| `171-KRN-0006` | `sysmon`'s per-CPU samples for every CPU (`idl/sysinfo.wit` 2.0), with `APP` for `top` and the load monitor | planned |
| [171-KRN-0005](../issues-done/171-KRN-0005-frame-pool-ranges.done) | 5. Frame pool ranges that grow with the firmware map | done (2026-10-06) |
