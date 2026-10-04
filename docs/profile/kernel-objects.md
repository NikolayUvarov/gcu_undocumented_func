# Kernel objects and limits — `x86-64/QEMU-0`

| Object | Limit | Created by | Paid from |
|---|---|---|---|
| Task (address space, stack 64 KiB, context, mailbox, exit page) | 32 tasks in total (a table limit; memory is the practical one: an application with a screen costs about 4 MiB of the 64 MiB arena at 1280×800, almost all of it the screen buffer); per owner: the spawner's task quota (each live child reserves 1 + its own task quota) | `SPAWN` (spawn privilege; services and boot images need the platform privilege) | Counted against the spawner's task quota; memory from the kernel heap (64 MiB, global) |
| Screen buffer | One per task spawned with `SPAWN_SCREEN` | `SPAWN` | Kernel heap |
| Capability slot | 96 per task: slots 1–22 fixed by convention (generation 0; receive and compositor targets), 23–95 handed out by the kernel with a 24-bit generation | Grants, `IPC` transfer, `ENDPOINT_CREATE`, `MEM_SHARE`, `PLATFORM_CAP`, `IPC_SAVE_REPLY` | Fixed per-task table |
| Endpoint | 127, created on demand and recycled when no capability refers to them; no endpoint has a number visible to tasks | `ENDPOINT_CREATE` (init's own: the kernel at boot) | The creator's endpoint quota (endpoints it created and still exist plus quotas delegated to its live children) |
| Private heap block | 32 blocks and 16 MiB per task | `ALLOC` | Kernel heap |
| Shared mapping | 48 MiB per task | `MEM_MAP` | Mapping quota per task; the memory belongs to its owner and is retained while referenced |
| Memory object | Objects and freed-but-referenced blocks: 16 MiB in total for `MEM_DETACH` | `MEM_DETACH`; `FREE` of a block others still hold | Charged to the detaching or freeing task's heap quota while it lives; after it exits, to nobody |
| DMA region | 8 MiB in total, 64 KiB aligned; kept for the platform's lifetime | `PLATFORM_CAP(PLATFORM_DMA)` (init) | Kernel heap |
| IRQ binding | One endpoint per line 1–15 (not 2) | `IRQ_BIND` | Fixed table |
| Input queue | 64 event words per task (key, modifiers, press/release, character, legacy byte), oldest dropped | `INPUT_EVENT` | Per task |
| Log and console queues | 4096 bytes each per task, oldest dropped; the unread console output of the last focused or screenless task that exited is kept until the next such exit | `LOG` | Per task |
| Fault records | 16, ring buffer | User exceptions | Global |
| Notices for the focus owner | 8, further ones dropped | Focused task exits, attention key | Global |

## Capability kinds

Endpoint (read/write/grant/keep rights; keep allows minting a child with the read right; a 16-bit badge set once by `CAP_MINT` and reported to the receiver), memory, DMA, MMIO, I/O port range, IRQ line, one-time reply, and the privileges input, display, spawn, process control, observe (read-only statistics, `STAT`), platform and restart (spawning boot images only). Handles carry a generation for kernel-allocated slots (a generation wraps after 16 777 215 reuses of one slot). Every capability has an identity and a parent: kernel-created capabilities are roots, copies and `CAP_MINT` children are descendants of their source, a move keeps the identity. `CAP_MINT` narrows endpoint and memory rights (read, write, grant), port ranges and page-aligned memory, DMA and MMIO ranges; other kinds are copied unchanged, reply capabilities cannot be minted. `CAP_REVOKE` removes all descendants from every task and from blocked sends and unmaps every mapping made from them. A capability that is dropped, overwritten or lost at exit while something still derives from it is kept as a ghost node (a table of 256 allocated at boot), so its descendants stay revocable; beyond that limit a dropped subtree can no longer be revoked from above. If an affected task is running on another CPU, its page entries are cleared, that CPU gets a wake IPI and the revoking task waits (`FLUSH_WAIT`) until it has reloaded CR3; the emptied page tables are reclaimed when the task exits.

## Quotas

Every task has a task quota and an endpoint quota, delegated by its spawner at `SPAWN` and taken from the spawner's own (MC-3.13). The kernel gives `init` the root quota: 31 tasks and 127 endpoints (its own endpoint included). `init` gives `loader` 8 tasks (`MAX_APPS`, the application limit is init's policy) and 32 endpoints; `loader` gives every application 0 tasks and 4 endpoints; other services get none. A spawn or `ENDPOINT_CREATE` beyond the quota fails with `ERR_LIMIT`. A child's reservation returns to its spawner when the child exits.

## Gaps

- Memory is not part of the quotas: tasks, private heaps and DMA regions are paid from the global kernel heap; per-task limits (heap 16 MiB, shared mappings 48 MiB) bound a single task, not an owner's subtree.
- Capability slots are a fixed per-task table rather than a quota.
- No CPU budgets or scheduling contexts (roadmap C7).

## Statistics

`STAT` copies bounded snapshots under the scheduler lock: at most 32 tasks, 4 CPUs (up to 8), 127 endpoints, 15 IRQ lines, the enumerated PCI functions, 96 capability slots per task, and the firmware memory map (up to 16 pages of entries). A `VMAP` walk visits only present page tables (at most 128 per task) and adds the guard page below the stack. On request (`STAT_MEMORY` with argument 1) the largest free block of the arena is found by at most 15 trial allocations of halving size, each freed at once. A holder of a capability in the records (an IRQ line, a device, an endpoint's server) is the task with the most recently derived copy, so a driver rather than `init`, which keeps the copies it granted. Records grow only at the end (`STAT_VERSION` 2 since issue 075). The kernel counts:
- per task: run time (TSC, at every switch), runs, ticks, system calls, IPC sends and receives;
- per CPU: busy and idle time, interrupts, switches;
- per endpoint: messages, `ERR_BUSY` refusals, timeouts;
- per IRQ line: interrupts.

