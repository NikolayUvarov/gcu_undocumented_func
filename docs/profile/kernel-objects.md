# Kernel objects and limits — `x86-64/QEMU-0`

| Object | Limit | Created by | Paid from |
|---|---|---|---|
| Task (address space, stack 64 KiB, context, mailbox, exit page) | 20 tasks in total; per owner: the spawner's task quota (each live child reserves 1 + its own task quota) | `SPAWN` (spawn privilege; services and boot images need the platform privilege) | Counted against the spawner's task quota; memory from the kernel heap (64 MiB, global) |
| Screen buffer | One per task spawned with `SPAWN_SCREEN` | `SPAWN` | Kernel heap |
| Capability slot | 32 per task: slots 1–9 fixed by convention, 10–31 handed out by the kernel | Grants, `IPC` transfer, `ENDPOINT_CREATE`, `MEM_SHARE`, `PLATFORM_CAP`, `IPC_SAVE_REPLY` | Fixed per-task table |
| Endpoint | 64: 1–15 reserved for system services (minted only by `init`), 16–63 created on demand and recycled when no capability refers to them | `ENDPOINT_CREATE`, `PLATFORM_CAP` | The creator's endpoint quota (endpoints it created and still exist plus quotas delegated to its live children) |
| Private heap block | 32 blocks and 16 MiB per task | `ALLOC` | Kernel heap |
| Shared mapping | 48 MiB per task | `MEM_MAP` | Mapping quota per task; the memory belongs to its owner and is retained while referenced |
| DMA region | 8 MiB in total, 64 KiB aligned; kept for the platform's lifetime | `PLATFORM_CAP(PLATFORM_DMA)` (init) | Kernel heap |
| IRQ binding | One endpoint per line 1–15 (not 2) | `IRQ_BIND` | Fixed table |
| Input queue | 128 bytes per task, oldest dropped | `INPUT_EVENT` | Per task |
| Log and console queues | 4096 bytes each per task, oldest dropped | `LOG` | Per task |
| Fault records | 16, ring buffer | User exceptions | Global |
| Notices for the focus owner | 8, further ones dropped | Focused task exits, attention key | Global |

## Capability kinds

Endpoint (read/write/grant rights), memory, DMA, MMIO, I/O port range, IRQ line, one-time reply, and the privileges input, display, spawn, process control and platform. Rights narrowing exists only for endpoints. There is no generation, derivation tree or revocation (roadmap C1–C2).

## Quotas

Every task has a task quota and an endpoint quota, delegated by its spawner at `SPAWN` and taken from the spawner's own (MC-3.13). The kernel gives `init` the root quota: 19 tasks and 48 endpoints. `init` gives `loader` 8 tasks (`MAX_APPS`, the application limit is init's policy) and 32 endpoints; `loader` gives every application 0 tasks and 4 endpoints; other services get none. A spawn or `ENDPOINT_CREATE` beyond the quota fails with `ERR_LIMIT`. A child's reservation returns to its spawner when the child exits.

## Gaps

- Memory is not part of the quotas: tasks, private heaps and DMA regions are paid from the global kernel heap; per-task limits (heap 16 MiB, shared mappings 48 MiB) bound a single task, not an owner's subtree.
- Capability slots are a fixed per-task table rather than a quota.
- No CPU budgets or scheduling contexts (roadmap C7).
