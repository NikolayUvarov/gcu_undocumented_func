# Kernel objects and limits — `x86-64/QEMU-0`

| Object | Limit | Created by | Paid from |
|---|---|---|---|
| Task (address space, stack 64 KiB, context, mailbox, exit page) | 24 tasks in total; per owner: the spawner's task quota (each live child reserves 1 + its own task quota) | `SPAWN` (spawn privilege; services and boot images need the platform privilege) | Counted against the spawner's task quota; memory from the kernel heap (64 MiB, global) |
| Screen buffer | One per task spawned with `SPAWN_SCREEN` | `SPAWN` | Kernel heap |
| Capability slot | 32 per task: slots 1–11 fixed by convention (generation 0; receive and compositor targets, launcher grants in 10–11), 12–31 handed out by the kernel with a 24-bit generation | Grants, `IPC` transfer, `ENDPOINT_CREATE`, `MEM_SHARE`, `PLATFORM_CAP`, `IPC_SAVE_REPLY` | Fixed per-task table |
| Endpoint | 63, created on demand and recycled when no capability refers to them; no endpoint has a number visible to tasks | `ENDPOINT_CREATE` (init's own: the kernel at boot) | The creator's endpoint quota (endpoints it created and still exist plus quotas delegated to its live children) |
| Private heap block | 32 blocks and 16 MiB per task | `ALLOC` | Kernel heap |
| Shared mapping | 48 MiB per task | `MEM_MAP` | Mapping quota per task; the memory belongs to its owner and is retained while referenced |
| Memory object | All objects and freed-but-referenced blocks: 16 MiB in total | `MEM_DETACH` | Kernel heap; not charged to an owner |
| DMA region | 8 MiB in total, 64 KiB aligned; kept for the platform's lifetime | `PLATFORM_CAP(PLATFORM_DMA)` (init) | Kernel heap |
| IRQ binding | One endpoint per line 1–15 (not 2) | `IRQ_BIND` | Fixed table |
| Input queue | 64 key events per task (32-bit words decoded in ring 3), oldest dropped | `INPUT_EVENT` | Per task |
| Log and console queues | 4096 bytes each per task, oldest dropped; the unread console output of the last focused or screenless task that exited is kept until the next such exit | `LOG` | Per task |
| Fault records | 16, ring buffer | User exceptions | Global |
| Notices for the focus owner | 8, further ones dropped | Focused task exits, attention key | Global |

## Capability kinds

Endpoint (read/write/grant/keep rights; keep allows minting a child with the read right; an optional 16-bit badge, set once by `CAP_MINT` on a child of an unbadged capability, kept by copies and children, reported to the receiver with every message sent through it), memory, DMA, MMIO, I/O port range, IRQ line, one-time reply, and the privileges input, display, spawn, process control, observe (read-only statistics, `STAT`) and platform. Handles carry a generation for kernel-allocated slots (a generation wraps after 16 777 215 reuses of one slot). Every capability has an identity and a parent: kernel-created capabilities are roots, copies and `CAP_MINT` children are descendants of their source, a move keeps the identity. `CAP_MINT` narrows endpoint and memory rights (read, write, grant), port ranges and page-aligned memory, DMA and MMIO ranges; other kinds are copied unchanged, reply capabilities cannot be minted. `CAP_REVOKE` removes all descendants from every task and from blocked sends and unmaps every mapping made from them. If an affected task is running on another CPU, its page entries are cleared, that CPU gets a wake IPI and the revoking task waits (`FLUSH_WAIT`) until it has reloaded CR3; the emptied page tables are reclaimed when the task exits.

## Quotas

Every task has a task quota and an endpoint quota, delegated by its spawner at `SPAWN` and taken from the spawner's own (MC-3.13). The kernel gives `init` the root quota: 23 tasks and 63 endpoints (its own endpoint included). `init` gives `loader` 8 tasks (`MAX_APPS`, the application limit is init's policy) and 32 endpoints; `loader` gives every application 0 tasks and 4 endpoints; other services get none. A spawn or `ENDPOINT_CREATE` beyond the quota fails with `ERR_LIMIT`. A child's reservation returns to its spawner when the child exits.

## Observation

`STAT` (system call 48) copies versioned fixed-size records into the caller's buffer under the scheduler lock; the work is bounded by the table sizes (24 tasks, 63 endpoints, 31 slots, 15 IRQ lines, the PCI functions, at most 170 firmware ranges). The kernel accounts for it: time on each CPU is measured with the TSC at every switch (busy, idle, per-task run time), interrupts per CPU and per line, messages, `ERR_BUSY` rejections and timeouts per endpoint. The bootloader hands over the UEFI memory map in the second handoff page (`BootInfo.memory_map`); tasks get a null pointer.

## Gaps

- Memory is not part of the quotas: tasks, private heaps and DMA regions are paid from the global kernel heap; per-task limits (heap 16 MiB, shared mappings 48 MiB) bound a single task, not an owner's subtree.
- Capability slots are a fixed per-task table rather than a quota.
- No CPU budgets or scheduling contexts (roadmap C7).
