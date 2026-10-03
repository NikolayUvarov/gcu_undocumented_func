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

Status: **met** (implemented and tested within this profile), **partial**, **not met**, **not claimed** (outside this profile). Requirement IDs are `MC-<article>.<clause>` of Constitution v1.6.

| Requirement | Status | Notes |
|---|---|---|
| MC-1.1, 1.3 minimal kernel, drivers outside | partial | Drivers, file system, loader, audio, TTS, command shell and service policy run in ring 3. The kernel still parses ELF images for `SPAWN` and enumerates PCI (discovery, not policy). |
| MC-1.4 policy separate from mechanism | met (resources) | `init` decides what each service gets; the kernel validates every resource it mints (legacy port allowlist, enumerated BARs, IRQ lines, reserved endpoints) regardless of `init`'s correctness. |
| MC-1.5 DMA boundary | **not met — declared** | No IOMMU: `ahci`, `usb_storage`, `audio_gw` and their devices are in the TCB of every memory-isolation guarantee ([tcb.md](tcb.md)). Isolation from them is not claimed. |
| MC-1.7 accounted creation | partial | Tasks and endpoints are charged to an owner's quota delegated at spawn ([kernel-objects.md](kernel-objects.md)); private heaps and shared mappings have per-task limits; DMA has a global limit. Memory is not charged to owners. |
| MC-1.8 reuse without residue | partial | All task memory is zeroed on allocation; freed memory is not reused while any mapping or capability refers to it. Devices are not reset on driver restart. |
| MC-2.1 isolated state | met | Private page tables; sharing only through memory capabilities. |
| MC-2.3 typed, versioned interfaces | not met | Protocols are numeric conventions in `common/abi.rs`; no IDL (roadmap C5). |
| MC-2.5 bounded queues, back-pressure | partial | IPC is a rendezvous (no kernel message queue); waiting senders are bounded by the task count. No cancellation contract. |
| MC-2.6 transfer modes | **not met** | Only shared read-write memory capabilities exist (`MEM_SHARE`): effectively `SHARE_RW` between the parties. COPY/MOVE/SHARE_RO/LEASE are roadmap C4. |
| MC-3.1, 3.2 explicit, unforgeable capabilities | partial | Capabilities live in a kernel table and are named by slot index. Slots have **no generation**: after a slot is freed and reused, an old index names the new capability (roadmap C1). |
| MC-3.3 no implicit authority for new domains | met | A new task gets exactly the spawner's grant list (`SPAWN`). |
| MC-3.4–3.6 copy/move/attenuate/revoke | not met | Only transfer with endpoint-rights narrowing and `CAP_DROP`; no derivation tree, no revocation (roadmap C2). |
| MC-3.7 no identity bypass | met | Every privileged system call checks a capability; no PID or name grants rights. |
| MC-3.12 end of initial distribution | partial | Bootstrap authority is only `init` ([bootstrap.md](bootstrap.md)), but `init` keeps the platform privilege for service restarts. |
| Article 4 storage | not claimed | FAT volumes are read-only external media; no content-addressed store. |
| MC-5.1–5.5 budgets | not met | Round-robin scheduling without budgets; task and endpoint quotas per owner, memory limits per task. |
| MC-5.6 explicit clocks | met (measurement) | Monotonic `CLOCK` with stated resolution, calendar time as a separate service; deadlines still have 10 ms granularity. See [clocks.md](clocks.md). |
| MC-6.1, 6.2 fault containment | met (ring 3) | A user exception terminates only that task, is recorded (`faults`) and its resources are reclaimed; kernel exceptions halt the system. |
| MC-6.4–6.9 supervision | not met | No supervisor, restart budget or instance generation; `init` restarts a service only on request (roadmap C6). |
| Articles 7, 8 | not claimed | Single node; no safety plane. |
| Article 9 boot and update | not met — declared | Boot images are not signed or measured; the bootloader loads whatever is on the boot volume. |
| MC-10.2 observability under authority | met | Task logs, console output, task list, faults and CPU data are readable only with the process-control privilege (the shell). |
| MC-10.5 side channels | not claimed | No mitigation is claimed. |
| MC-11.1 explicit ABI | partial | The ABI is a `repr(C)` mailbox and constants in `common/abi.rs`; there is no versioning. |
| MC-11.3, 11.11 external formats in adapters | partial | FAT and USB/SCSI parsing run in ring 3 services with only their device capabilities; ELF parsing of applications runs in the kernel. |
