# Platform profile `x86-64/QEMU-0`

**Version:** 0.1 (2026-10-03) · **Constitution:** [v1.6](../../constitution/EN/MIND_CORE_Constitution_v1.6.md), stage 0 · **Roadmap:** [S0](../../ROADMAP.md)

This profile states what the current implementation is, what it guarantees, under which assumptions, and what it does **not** claim (MC-12.1, MC-12.3). It describes the code as of the commit that contains this file; a change that alters a statement here updates this profile in the same commit (MC-12.9).

| Document | Content |
|---|---|
| [threat-model.md](threat-model.md) | Assets, adversaries, fault model, what is out of scope |
| [tcb.md](tcb.md) | Trusted computing base for each guarantee |
| [kernel-objects.md](kernel-objects.md) | Kernel objects, capability kinds, limits and who pays for them |
| [clocks.md](clocks.md) | Time sources, resolution, what is not provided |
| [bootstrap.md](bootstrap.md) | Bootstrap authority: what the kernel gives `init`, what `init` hands out |
| [evidence.md](evidence.md) | Which tests support which statement |

## Platform

- x86-64, UEFI (OVMF in QEMU), 1–8 xAPIC CPUs, no SMT siblings in the tested configuration (`-smp N,cores=N,threads=1`).
- QEMU `pc` machine with IDE, optionally AHCI, xHCI with USB mass storage, AC97; physical machines boot from the USB image but are not part of the evidence.
- **No IOMMU is used.** Every device that can do DMA can read and write all physical memory.
- One node; no network.

## Conformance

Status: **met** (implemented and tested within this profile; each met row names its evidence, details in [evidence.md](evidence.md)), **partial**, **not met**, **not claimed** (outside this profile). Requirement IDs are `MC-<article>.<clause>` of Constitution v1.6.

| Requirement | Status | Notes |
|---|---|---|
| MC-1.1, 1.3 minimal kernel, drivers outside | partial | Drivers, file system, loader, audio, TTS, command shell and service policy run in ring 3. The kernel still parses ELF images for `SPAWN` and enumerates PCI (discovery, not policy). |
| MC-1.4 policy separate from mechanism | partial | `init` decides what each service gets; the kernel validates every resource it mints (legacy port allowlist, enumerated BARs, IRQ lines) regardless of `init`'s correctness. Evidence: `services` suite (services start with exactly init's grants). The kernel's refusal of out-of-profile `PLATFORM_CAP` arguments has no test (only `init` holds the privilege). |
| MC-1.5 DMA boundary | **not met — declared** | No IOMMU: `ahci`, `usb_storage`, `audio_gw` and their devices are in the TCB of every memory-isolation guarantee ([tcb.md](tcb.md)). Isolation from them is not claimed. |
| MC-1.7 accounted creation | partial | Tasks and endpoints are charged to an owner's quota delegated at spawn ([kernel-objects.md](kernel-objects.md)); private heaps and shared mappings have per-task limits; DMA has a global limit. Memory is not charged to owners. |
| MC-1.8 reuse without residue | partial | All task memory is zeroed on allocation; freed memory is not reused while any mapping or capability refers to it. Devices are not reset on driver restart. |
| MC-2.1 isolated state | met | Private page tables; sharing only through memory capabilities. Evidence: `isolation` suite cases r, w, t, n; host test `same_virtual_address_maps_private_frames_in_distinct_cr3s`. |
| MC-2.3 typed, versioned interfaces | partial | MIND IDL v0 ([docs/idl](../idl/README.md)): typed, versioned WIT-subset interfaces with size limits and capability kinds, generated bindings, schema checks in the receiver. Only `rtc` uses it; the other protocols are numeric conventions in `common/abi.rs` (roadmap C8). Ordering and session contracts are not described. |
| MC-2.5 bounded queues, back-pressure | met | IPC is a rendezvous (no kernel message queue); at most `ENDPOINT_QUEUE` (4) senders wait per endpoint, a further send fails with `ERR_BUSY`. Every send, call and receive may carry a timeout; expiry withdraws the operation without a trace, and a later reply to that call fails with `ERR_PEER`. Timeouts have 10 ms granularity. Evidence: `isolation` cases `k` (timeouts), `q` (queue bound with five senders), `j` (late reply). |
| MC-2.6 transfer modes | partial | COPY: two data words per message. MOVE: a detached memory object (`MEM_DETACH`) has one writable owner and is transferred only with `CAP_TRANSFER_MOVE`. SHARE_RO: a read-only capability the kernel reports as sealed (no writer, no DMA). LEASE: a copy ended by `CAP_REVOKE`, which unmaps it with a completion point. Existing services still share heap blocks read-write (`MEM_SHARE`, roadmap C8). |
| MC-3.1, 3.2 explicit, unforgeable capabilities | met (kernel-allocated slots) | Capabilities live in a kernel table and are named by handles `slot \| generation << 8`. A slot the kernel hands out gets a new generation when freed, so a stale handle is rejected. Fixed slots 1–9 are named by their owner and overwritten only by the owner's own receive. Evidence: `isolation` case `k` (stale handle after slot reuse, mint and revoke). |
| MC-3.3 no implicit authority for new domains | met | A new task gets exactly the spawner's grant list (`SPAWN`). Endpoints have no global names; a service is reachable only through a capability `init` derived for the client. Evidence: `isolation` case `k` (an application holds no privilege: input, ports, spawn, platform, control refused). |
| MC-3.4–3.6 copy/move/attenuate/revoke | partial | Copy and move are distinct; `CAP_MINT` attenuates endpoint rights and port/memory ranges; `CAP_REVOKE` removes all descendants (including one in a blocked send) before it returns. Mappings made from a revoked capability are removed before `CAP_REVOKE` returns. |
| MC-3.7 no identity bypass | met | Every privileged system call checks a capability; no PID or name grants rights. Evidence: `isolation` case `k` (privileged calls refused without the capability). |
| MC-3.12 end of initial distribution | met | Bootstrap authority is only `init` ([bootstrap.md](bootstrap.md)). Before `[INIT] READY` it gives up the platform privilege; it keeps the capabilities it handed to each service and a restart privilege that can only spawn boot images. Evidence: `services` suite (`PLATFORM PRIVILEGE DROPPED`, restarts afterwards). |
| Article 4 storage | not claimed | FAT volumes are read-only external media; no content-addressed store. |
| MC-5.1–5.5 budgets | partial | Scheduling contexts (C7): a CPU budget per period, enforced at the 10 ms tick, set by the lifecycle owner or process control (`SCHED_SET`); two bands — `init` and services run before applications, so the supervisor's reserve survives application overload. Evidence: `busy` suite (a 20 ms/100 ms budget keeps a busy loop near 20 %), `smp` suite (`init` restarts a service while every CPU is saturated). Not provided: deadlines, admission control, budgets below the tick, memory budgets per owner. |
| MC-5.6 explicit clocks | met (measurement) | Monotonic `CLOCK` with stated resolution, calendar time as a separate service; deadlines still have 10 ms granularity. See [clocks.md](clocks.md). Evidence: `services` suite (`clock` command). |
| MC-6.1, 6.2 fault containment | met (ring 3) | A user exception terminates only that task, is recorded (`faults`) and its resources are reclaimed; kernel exceptions halt the system. Evidence: `isolation` suite (each fault case leaves the other tasks running; heap baseline restored). |
| MC-6.4–6.9 supervision | partial | `init` is the lifecycle owner of every service: the kernel sends it exit notices (`TASK_WATCH`), it restarts a failed service at most 3 times in 60 s and then quarantines it until an explicit `RUN`. Sends queued for a dead server fail with `ERR_PEER` instead of reaching the next instance; clients tell instances apart by the server PID in replies. Without `init` the system halts. Evidence: `services` suite (restart, budget, quarantine, halt), `isolation` case `f` (queued send fenced). Before a driver restarts, its device is stopped (`DEVICE_STATE`) and its DMA region cleared (MC-6.3, `ahci` suite). The block client re-attaches to a new driver instance and repeats only idempotent reads (MC-6.6). Not done: idempotency rules for other protocols, lifecycle owners of applications beyond loader's accounting. |
| Articles 7, 8 | not claimed | Single node; no safety plane. |
| Article 9 boot and update | not met — declared | Boot images are not signed or measured; the bootloader loads whatever is on the boot volume. |
| MC-10.2 observability under authority | met | Task logs, console output, task list, faults and CPU data are readable only with the process-control privilege (the shell). Evidence: `isolation` case `k` (`TASK_KILL`, `FOCUS`, `HALT` refused to an application). Statistics (`STAT`, task list, CPU data, kernel heap, faults) need the observe privilege or process control; kill, focus, logs and console output need process control. `STAT` exports no task memory contents, no physical address of task memory and nothing usable as authority (endpoint indices are labels). |
| MC-10.5 side channels | not claimed | No mitigation is claimed. |
| MC-11.1 explicit ABI | partial | The ABI is a `repr(C)` mailbox and constants in `common/abi.rs`; there is no versioning. |
| MC-11.3, 11.11 external formats in adapters | partial | FAT and USB/SCSI parsing run in ring 3 services with only their device capabilities; ELF parsing of applications runs in the kernel. |
