# 174 — The PC's full capacity for programs: Intel, AMD and NVIDIA

**Type:** kernel (main task) · **Owner:** `KRN`, with `DRV` (GPU drivers) and `PRT` · **Priority:** P1 · **Status:** open (plan) · **Blocked by:** — · **Roadmap:** tracks A and H · **Constitution:** MC-1.2, MC-1.5, MC-5.1, MC-5.2, MC-12.1

The maintainer set the stage's goal on 2026-10-09: develop the kernel for Intel and aarch64, test on the Intel MacBook Pro and on ordinary Intel and AMD PCs, and use 100 % of what Intel, AMD and NVIDIA hardware offers to give programs, and the mind that will live in this system, their resources. Apple Silicon is parked (TRACKS.md).

## Problem

What MIND Core uses of a PC today, checked in the code on 2026-10-09:

- **CPUs.** Every CPU the MADT lists is used (up to 255 xAPIC IDs, x2APIC included: 171, 211-PRT-0002). The per-task vector state is limited to x87, SSE and AVX: `XCR0 = 0b111` and a 1 KiB save area (`kernel/src/arch/x86_64/mmu.rs:131`, `context.rs:6`). AVX-512 (Intel Xeon, AMD Zen 4 and 5) and AMX (Xeon from Sapphire Rapids) are not enabled, so a program that uses them faults (#UD).
- **Speed and idle.** An idle CPU waits in `HLT` (`kernel/src/arch/x86_64/cpu.rs:220–374`). Deep C-states through `MWAIT` are not used, and no frequency is asked for: no Intel HWP, no AMD CPPC. The CPU runs at whatever performance state the firmware left, so turbo frequencies may never be reached under load.
- **Timer.** A periodic LAPIC tick (211-PRT-0003); the TSC deadline mode is not used.
- **Memory.** The kernel maps RAM above 4 GiB with 2 MiB pages, but a program's memory is mapped in 4 KiB pages. A model of several GiB then costs a TLB miss every 4 KiB. NUMA is unknown to the kernel: no SRAT or SLIT is read, so memory is not allocated near the CPU that uses it.
- **DMA.** No IOMMU: every DMA driver is in the TCB (MC-1.5; ROADMAP K2, III-4).
- **aarch64 vector units.** Programs are built for `aarch64-unknown-none-softfloat`, and a task's saved context is 37 words with no FP/SIMD registers (`kernel/src/arch/aarch64/context.rs:8`). EL0 has FP/SIMD turned off, except during a firmware call (351-KRN-0028). So NEON, SVE and SME are never used.
- **What the machine is.** Nothing records it. A machine's CPUs, firmware, ACPI tables, PCI functions, GPUs and NPUs are known only from what the kernel and the drivers print. The kernel lines after the compositor takes the screen are lost on a machine without COM1 (211-KRN-0021).
- **GPUs.**
  - No GPU driver: the screen is the firmware's framebuffer (GOP), drawn by the CPU.
  - Intel, AMD and NVIDIA GPUs (`devices` lists them on the MacBook Pro: Intel HD 4000 and an NVIDIA GK107) compute nothing for programs.
  - Graphics memory is mapped uncached, not write-combining.

## Plan

First the report of what the machine is. Then the kernel steps, each of which gives programs more of the machine at once. Then the IOMMU, which must come before drivers that move a GPU's worth of memory by DMA are given it. Then the GPUs.

| Step | Task | What |
|---|---|---|
| 0 | [174-KRN-0038](174-KRN-0038-hardware-report.md) | **Know the machine.** Every boot writes a complete hardware report, `log:hwNNNN.txt` next to `bootNNNN.log`, with the ACPI tables as files beside it. It covers the CPUs (every CPUID leaf, topology, caches, microcode, frequencies, XSAVE components), the firmware (UEFI vendor and revision, SMBIOS, ACPI tables), memory (the firmware's map, NUMA), every PCI function (IDs, class, BARs, capabilities, PCIe link, config space) with GPUs and NPUs in full, and the kernel's own decisions. |
| 1 | [174-KRN-0037](174-KRN-0037-every-vector-state-component.md) | Every XSAVE component the CPU has, for programs: AVX-512 (opmask, ZMM_Hi256, Hi16_ZMM), AMX (XTILECFG, XTILEDATA), PKRU. The save area is sized from CPUID 0xD at boot and kept per task; compact XSAVE where the CPU has it. `BootInfo.cpu_features` says what is there. |
| 1b | (numbered when started) | **aarch64 vector state:** FP/SIMD (NEON) per task, then SVE and SME where the core has them; programs built for a hard-float target. |
| 2 | (numbered when started) | **Performance states:** Intel HWP (IA32_PM_ENABLE, HWP_REQUEST: the whole range, the preference set to performance) and AMD CPPC; idle in the deepest MWAIT C-state CPUID leaf 5 offers, with the exit latency bounded for the system band (MC-5.3). Each CPU's effective frequency (APERF/MPERF) shown in `STAT_CPUS` and `top`. |
| 3 | (numbered when started) | **Large pages for programs:** 2 MiB, and 1 GiB where the CPU has them, for large memory objects (models, frame buffers); 2 MiB-aligned runs from the frame pool; still charged to the program's quota (MC-5.1). |
| 4 | (numbered when started) | **NUMA:** the SRAT and SLIT read; a frame pool and a CPU set per node; memory taken near the CPU of the task that asks; nodes in `STAT` and `top`. |
| 5 | (numbered when started) | **IOMMU:** Intel VT-d (DMAR) and AMD-Vi (IVRS); a DMA domain per driver, default deny, holding only the DMA regions it was granted; interrupt remapping. This closes MC-1.5's gap on PCs (ROADMAP III-4). |
| 6 | (numbered when started) | **Large BARs:** 64-bit BARs above 4 GiB and resizable BARs mapped as granted; write-combining (PAT) for graphics memory and the framebuffer. |
| 7 | `DRV` tasks | **A GPU service for computation**, one vendor first, behind one interface (`idl/accel.wit`: memory objects, queues, fences, budgets per program, MC-5.1). Order by openness: (a) **AMD** (register documentation; the Linux `amdgpu` driver is MIT-licensed, usable as reference; firmware blobs redistributable with their licence in THIRD_PARTY.md; kernels compiled with LLVM's AMDGPU target); (b) **NVIDIA** from Turing on (the GSP firmware and NVIDIA's open kernel modules, dual MIT/GPL, as reference; kernels compiled ahead of time; CUDA's runtime is closed and not a goal; the MacBook Pro's GK107 predates GSP and is out of scope); (c) **Intel** graphics (public PRMs, GuC firmware). |
| 8 | `DRV` tasks | **NPUs** behind the same accelerator interface: Intel's NPU (Meteor Lake and later; PCI class 12:00) and AMD's XDNA (Ryzen AI). Their Linux drivers are GPL, so they are written again from documentation and the firmware interfaces. The hardware report (step 0) records them in full first. |

Each step adds the lines of `docs/profile` it changes and is tested on the machines it names (MC-12.1). QEMU's TCG emulates neither AVX-512, AMX, HWP, NUMA hardware nor GPUs, so the real machines are the evidence for those parts. QEMU covers what does not change, and the parts it emulates (`-numa`, `intel-iommu`, `amd-iommu`).

## Acceptance criteria

On the maintainer's Intel and AMD machines, recorded in the profile per machine:

- a program uses AVX-512 and AMX where the CPU has them;
- under load the CPUs reach their turbo frequency, and an idle machine sits in deep C-states;
- a model of several GiB is mapped with 2 MiB pages;
- on a NUMA machine memory is allocated on the requesting CPU's node;
- every DMA driver is confined by the IOMMU;
- a program runs a compute kernel on one vendor's GPU through the accelerator service;
- every boot leaves a complete hardware report on the log partition. GPUs and NPUs are described in it in full.

## Related

[171](../issues-done/171-limits-from-the-hardware.done) (limits from the hardware), [211](211-intel-pc-from-a-sata-ssd.md) (the first real PCs), [550](550-network-on-real-hardware.md) (network cards), [knowledge/06](../knowledge/06-apple-security-lessons.md) (security proposals: IOMMU, SMEP/SMAP, measured boot).
