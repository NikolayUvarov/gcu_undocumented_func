# 171 — Limits from the hardware: no fixed caps on tasks, endpoints, CPUs, capability slots or RAM

**Type:** kernel · **Owner:** kernel track · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Roadmap:** K4 (accounted kernel objects: memory is still paid from the kernel heap), C1 · **Constitution:** MC-1.7, MC-3.13, MC-5.1

## Problem

Asked by the user (2026-10-06): MIND Core is meant to use the most of the machine it runs on, so its limits must come from the hardware alone. Today the kernel has fixed caps that no machine sets:

- 32 tasks for the whole system (`MAX_TASKS`), of which services take about 20;
- 8 applications (`MAX_APPS`, init's policy for loader's task quota);
- 128 IPC endpoints (`ENDPOINTS`), 4 for each application (`APP_ENDPOINTS`);
- 8 CPUs (`cpu::MAX`);
- 96 capability slots a task (`CAP_SLOTS`);
- 16 ranges of free RAM in the frame pool (`RANGES`);
- on x86-64, RAM below 4 GiB only: the kernel's identity map ends there (`IDENTITY_END`), so a machine with 8 GiB uses about 3.

Found on the way (step 6):

- a program's memory: at most 1024 MiB (`APP_MEMORY_MAX` in loader), in a heap window of 928 MiB (`USER_END`), in at most 64 blocks (`HEAP_MAX_BLOCKS`); 256 MiB of shared mappings a task and 256 MiB of detached objects in all;
- 8 MiB of DMA regions in all (`DMA_LIMIT`) and the 64 MiB kernel arena;
- the monitors' interface: `sysinfo` lists 40 tasks, 128 endpoints and 8 CPUs, and a load sample counts tasks in a byte.

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
6. **A program's memory** up to what the machine has: the heap window, the block count and the global caps above from memory.

Each step is its own commit, with the docs/profile statements (kernel-objects.md, bootstrap.md) changed with it.

## Progress

- **Step 1 — done (2026-10-06).**
  - The x86-64 kernel maps the free RAM above 4 GiB in 2 MiB pages, up to 512 GiB (root entry 0; `mmu::RAM_END`), a page directory per gigabyte that has any. The frame pool takes the same pages (`mmu::mapped`, shared by both).
  - The pool now serves the highest range first, so memory below 4 GiB stays free longest, and the identity map is built before the pool.
  - `03_run_qemu.sh`, `03_run_qemu_wsl.sh` and the Windows launchers take `MIND_MEMORY` (default `512M`).
  - Tested: QEMU x86-64 with 6 GiB: the pool has 6018 MiB, 3071 MiB of it above 4 GiB; `memtest` writes and reads back a 144 MiB heap; the `normal`, `display`, `net` and `vfs` suites pass. aarch64 with 6 GiB: 6033 MiB, 3044 MiB above 4 GiB. CI runs the x86 group "RAM above 4 GiB".
  - Left over, outside step 1:
    - on x86, RAM between 3 and 4 GiB (real machines can have it) is still mapped uncached;
    - x86 RAM above 512 GiB needs a second kernel root entry;
    - the kernel arena is a fixed 64 MiB (step 2 moves task structures out of it).

- **Step 2, kernel and shell — done (2026-10-06).**
  - The task table and the endpoint table grow as needed and shrink when their last entries are reclaimed; a slot past the end reads as empty, so a kept (slot, PID) finds no task. `MAX_TASKS` and the 128 endpoints are gone.
  - A task's structure, context, info and mailbox page and exit page come from the frame pool and are charged to its spawner with its image, stack and screen (`StatTask::spawn_bytes`). Its page tables come from the frame pool, uncharged: at most one per 2 MiB it maps.
  - `SPAWN_QUOTA_UNBOUNDED` (0xFFFF) is a task or endpoint quota without a count, given only by a spawner without one. init has the root quota without counts and gives one to loader, so `MAX_APPS` is gone. Amendment to the plan: loader still gives each application 4 endpoints. An application's endpoints are bounded by its capability slots until step 4 pays for them from memory.
  - `STAT` version 3: `argument` is the first record of `STAT_TASKS` and `STAT_ENDPOINTS` (`mind::stat::each` reads every page); `tasks_limit` and `endpoints_limit` are 0. The shell's `ps`, `quotas` and `stat <pid>` see every task; the monitors show no limit and `-` for a quota without a count.
  - An endpoint named by a watch or a blocked IPC is not handed out again while they last.
  - Tested in QEMU with 4 CPUs and 512 MiB: 80 clocks with screens at once on x86-64, 171 on aarch64 (its screen is smaller); the next one is refused with `OUT OF MEMORY` and the system goes on; the arena and the frame pool come back. Also the `services`, `memory`, `heap`, `isolation` and `tools` suites on x86-64.
  - Left for step 2: the `sysinfo` interface (`sysmon`, `top`, the console's `ps`, `logd`'s names) still reads 40 tasks and 128 endpoints.

## Acceptance criteria

- **QEMU x86-64, 6 GiB:** the frame pool reports the RAM above 4 GiB, and a program allocates there. *(Met by step 1.)*
- **QEMU, both architectures:** more than 32 applications run at once (console programs; screens cost 4 MiB each). The limit comes only when memory runs out, as a clean refusal (`TASK LIMIT`/`NO MEMORY`) with the system left working, and the recovery reserve keeps the services restartable.
- **QEMU with 16 CPUs (x86, TCG):** all come online.
- **Host and QEMU tests:** a task with more than 96 capabilities, and the STAT pages.

## Related

[150](../issues-done/150-user-memory-beyond-the-arena.done), [168](../issues-done/168-task-memory-charged-to-spawner.done), [169](../issues-done/169-recovery-reserve.done), [024](../issues-done/024-accounted-kernel-objects.done).
