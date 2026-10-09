# Platform profile `aarch64/QEMU-virt-0`

**Version:** 1.3 (2026-10-06; 1.2 → 1.3: pin controllers from the DSDT and SSDTs — issue 206; 1.1 → 1.2: USB keyboards, mice and tablets through `usb_host` and `usb_hid` — issue 164; no console UART or RTC at `virt`'s addresses on other boards — issue 205; 1.0 → 1.1: the board's layout from ACPI, RAM and devices above 4 GiB, GICv2, NVMe — issue 205) · **Constitution:** [v1.6](../../../constitution/EN/MIND_CORE_Constitution_v1.6.md), stage 0 · **Roadmap:** track H (issues 201–205)

This is the second platform of MIND Core. It shares the kernel's generic part, every service's source and the system-call interface with [`x86-64/QEMU-0`](../README.md); what differs is the architecture layer (`kernel/src/arch/aarch64/`, `libmind/src/arch/aarch64.rs`), the bootloader's few architecture lines and the devices. Like the x86 profile it states what the implementation is, guarantees and does not claim (MC-12.1, MC-12.3), for the code of the commit that contains it; a change that alters a statement here updates it in the same commit (MC-12.9). Where this profile says nothing, the x86 profile's statement holds unchanged: its [conformance table](../README.md#conformance), [kernel objects](../kernel-objects.md), [clocks](../clocks.md), [bootstrap](../bootstrap.md) and [memory transfers](../README.md#memory-transfers-appendix-b2).

| Document | Content |
|---|---|
| [tcb.md](tcb.md) | The trusted computing base: what differs from x86 (GIC, ITS, firmware, ACPI tables, PSCI, the VirtIO drivers) |
| [threat-model.md](threat-model.md) | Assets, adversaries and faults as on x86, and the differences |
| [evidence.md](evidence.md) | Which tests on `virt` support which statement |

## Building and running

```bash
ARCH=aarch64 ./02_build.sh            # or scripts/build_aarch64.sh [--fixtures]: aarch64_root/
./03_run_qemu_aarch64.sh              # the shell on this terminal, the screen in a window
python3 tests/aarch64_smoke.py        # boot and fault containment (after --fixtures)
python3 tests/qemu_smoke.py --arch aarch64   # normal, shell, vfs, net, tls, busy, smp
```

Needs the Rust targets of `rust-toolchain.toml`, `qemu-system-aarch64`, AAVMF and the iPXE ROMs (Debian/Ubuntu: `qemu-system-arm qemu-efi-aarch64 ipxe-qemu`); the vfs suite needs `dosfstools` and `mtools`. CI: the `aarch64` jobs of `.github/workflows/ci.yml`.

## Machine

QEMU `virt,gic-version=3,highmem=off`, `-cpu max`, `-smp 4`, 512 MiB, AAVMF (EDK2) firmware, a `ramfb` display for the firmware's graphics output, and VirtIO PCI devices: the boot disk on `virtio-blk`, `virtio-net`, `virtio-keyboard` and `virtio-tablet`. `highmem=off` keeps the PCIe ECAM below 4 GiB, inside the kernel's identity map; with it above, the kernel reports `PCI: ECAM ABOVE 4 GIB, NOT USED` and runs without PCI.

The kernel reads the machine's layout from ACPI (`arch/aarch64/board.rs`, printed as `MIND CORE KERNEL: BOARD …`): the ECAM from the MCFG; the CPUs (MPIDRs), the GIC's version and addresses (distributor, redistributors — each CPU finds its own by its affinity in `GICR_TYPER` — or GICv2 CPU interface, ITS or GICv2m frame) from the MADT; the console UART and its interrupt from the SPCR; the virtual timer's PPI from the GTDT; PSCI's conduit (HVC or SMC) from the FADT. Only the PL031 stays at `virt`'s address (`0x0901_0000`, SPI 2): it is not in ACPI's static tables. `virt`'s values are the defaults: GICv3 at `0x0800_0000`, ITS at `0x0808_0000`, redistributors from `0x080A_0000`, PL011 at `0x0900_0000` (SPI 1).

Without ACPI (`virt,acpi=off`, as a board with only a device tree), the firmware hands over a flattened device tree. The bootloader passes its address in `BootInfo.device_tree`, and the kernel checks its header (`MIND CORE KERNEL: DEVICE TREE AT …`, 210-KRN-0029). The kernel does not read the board from it yet (210-APL-0002). Such a machine boots to init with `virt`'s defaults, no PCI and no console, and only its screen shows the logs.

Variants tested as stand-ins for boards (issue 205): `highmem=on` with 6 GiB (RAM, ACPI tables, the ECAM and 64-bit PCI windows above 4 GiB), `gic-version=2` (a GICv2 with GICv2m, the Raspberry Pi 4's kind of GIC), and the boot disk on NVMe.

## What the kernel does here

| Mechanism | aarch64 | x86-64 counterpart |
|---|---|---|
| Privilege | Tasks at EL0, the kernel at EL1 | ring 3, ring 0 |
| System call | `svc #0`, the same mailbox ABI | `int 0x80` |
| Exceptions | EL1 vector table (`VBAR_EL1`); a per-CPU exception stack (`TPIDR_EL1`) for entries from tasks | IDT, TSS stacks |
| Address spaces | Stage 1, 4 KiB granule, 4 levels, 48-bit TTBR0: L0 entries 0 and 1 the kernel's identity map of the first TiB (EL1 only; RAM from the UEFI memory map normal memory, the rest device memory, in 1 GiB blocks or 2 MiB where a gigabyte holds both), the task window at L0 entry 255 (x86: 512 GiB); user pages `PXN`, data pages `UXN`, code read-only; the generic walk is `kernel/src/paging.rs` | 4-level page tables, NX |
| Interrupts | GICv3, or GICv2 with its CPU interface in memory and SGIs through `GICD_SGIR` (issue 205): SPI 32 + n is device line n (PCI INTA–D of slot s, pin p: line 3 + (s + p − 1) mod 4); MSI-X through the ITS, line 16 + n is LPI 8192 + n (event n of the device's requester ID, collection 0 on CPU 0), or through a GICv2m frame (line 16 + n is the frame's n-th SPI, edge-triggered); SGIs 1–3 for stop, tick and wake between CPUs | 8259 PIC, xAPIC, MSI-X |
| PCI | ECAM from the MCFG, the same enumeration, BARs and MSI-X tables (`kernel/src/pci.rs`; the architecture gives configuration access, the legacy line and the MSI message) | ports `0xCF8`/`0xCFC` |
| Platform devices | `PLATFORM_MMIO` hands out the board's UART and RTC by index (`PLATFORM_UART`, `PLATFORM_RTC`), `PLATFORM_IRQ` their lines (`arch/aarch64/platform.rs`); there are no I/O ports | ISA port ranges and lines 1–15 |
| Tick and clock | EL1 virtual timer, 100 Hz; `CNTVCT_EL0` at `CNTFRQ_EL0` for the monotonic clock | PIT, TSC |
| FP/SIMD | Disabled (`CPACR_EL1`): programs and kernel are soft-float, no FP state is saved yet | x87/SSE/AVX saved per task, with AVX-512 and AMX where the CPU has them (174-KRN-0037) |
| Entropy | `RNDR` when `ID_AA64ISAR0_EL1` lists it; the kernel tells tasks in `BootInfo.cpu_features` (EL0 cannot read ID registers) | `RDRAND` |
| CPUs | The boot CPU and the others the MADT lists (a table of 256; issue 171), started with PSCI `CPU_ON` into a trampoline that turns on the MMU with the boot CPU's MAIR, TCR, TTBR0 and SCTLR (its record and code cleaned to memory first); each has its exception stack and redistributor. The boot CPU's virtual timer ticks; the others get the tick as an SGI | INIT-SIPI-SIPI, local APIC timer IPIs |
| TLB | `TLBI VMALLE1IS`: a change of an address space is broadcast to every CPU by the instruction itself, no IPIs | reload of CR3 on the next switch |
| Reset and power off | PSCI `SYSTEM_RESET` and `SYSTEM_OFF` (`reboot`, `reboot --off`) | ACPI reset register, 0xCF9, 8042; no power off yet |
| Console | PL011: the kernel's lines, and every task's log until the shell gets the PL011 (`mind::dev::Uart`), then the shell's console as on COM1 | COM1, the shell |
| Instruction cache | Invalidated after the kernel writes program images and the exit page | coherent |

## Programs

The same sources, built for `aarch64-unknown-none-softfloat` as static PIEs with 4 KiB segments (`.cargo/config.toml` of each program); the loaders accept `R_AARCH64_RELATIVE`. `scripts/build_aarch64.sh` builds the bootloader and every crate of `02_build.sh` except the ISA drivers (`ata`, `ps2_kbd`, `audio_gw`), into `aarch64_root/`; `virtio_net` without its legacy (port I/O) interface. The bootloader leaves out the boot images it does not find, and init reports them as `NOT STARTED: NO IMAGE`.

## Devices and their services

| Device | Service | Notes |
|---|---|---|
| virtio-blk (PCI) | `virtio_blk` (both architectures) | the boot disk; modern interface, requests polled; vfs mounts FAT from it (`FROM VIRTIO`) |
| NVMe (PCI) | `nvme` (both architectures, issue 205) | a boot disk on NVMe (`FROM NVME`), as boards and servers have |
| virtio-net (PCI) | `virtio_net` | modern interface, MSI-X through the ITS (INTx without it) |
| xHCI (PCI) with USB keyboards, mice, tablets, hubs, mass storage | `usb_host`, `usb_hid`, `usb_storage` (both architectures, issue 164) | devices behind USB 2 hubs, plugged in and out at run time; the keyboard service is `usb_hid`'s when there is no VirtIO keyboard; the boards' only keyboards |
| virtio-keyboard, virtio-tablet (PCI) | `virtio_input` | up to two devices; keys go through the PS/2 decoder (layouts, Ctrl+Z, the keyboard service `idl/keyboard.wit`), the tablet gives absolute pointer events |
| PL011 | `shell` | its console, as COM1 on x86 |
| Pin controllers (PL061, BCM2711 GPIO) named in the DSDT or SSDTs | `gpio` (issue 207) | the kernel finds them (`MIND CORE KERNEL: PINS …`) and gives their registers to `init` by index; `gpio` serves `idl/gpio.wit`. QEMU `virt` with ACPI has none (its PL061 is replaced by the GED), so `gpio` is tested on the host only and has not run on hardware |
| PL031 | `rtc` | seconds since 1970, UTC |
| ramfb | `compositor` | the firmware's GOP framebuffer (800x600) |

## Conformance: differences from x86-64/QEMU-0

| Requirement | Status here | Difference |
|---|---|---|
| MC-1.5 DMA boundary | **not met — declared** | No SMMU is used (QEMU `virt` has none unless `iommu=smmuv3`): `virtio_blk`, `nvme`, `virtio_net`, `virtio_input`, `usb_host` and their devices can read and write all physical memory and are in the TCB of every memory guarantee. Unlike x86 the ITS keys each MSI by the device's requester ID, so a device can raise only the LPIs mapped for its own events; this does not help against DMA. |
| MC-2.6 transfer modes | as on x86 | `block.attach` also goes to `virtio_blk` (the same SHARE_RW adapter). |
| MC-5.1–5.5 budgets | as on x86 | Evidence: `busy` and `smp` suites on four CPUs. |
| MC-5.6 explicit clocks | met (measurement) | The generic timer's virtual count; calendar time from the PL031 (`rtc`), seconds since 1970 in UTC. |
| MC-6.1, 6.2 fault containment | met (EL0) | A task's synchronous exception (any exception class from EL0) ends only it; evidence: `aarch64_smoke.py` (kernel read, code write, stack execution, undefined instruction). A kernel exception halts the system. |
| MC-6.10–6.12 checkpoints | partial | As on x86: the checkpoint format and protocol of `mind::checkpoint`, with the pilot `tally`. Evidence: `store` suite on `virt`. |
| MC-10.5 side channels | not claimed | As on x86; no speculation barriers, no PAN (the kernel reaches task memory through its identity map only). |
| Article 4 storage | partial | As on x86: the block store runs at boot over `ramdisk#1`, with names that keep their history and can be removed, commits of several names, pins, quotas per owner and collection by reachability. Evidence: `store` and `storefaults` suites on `virt`. |
| Article 9 boot and update | partial | As on x86: `scripts/build_aarch64.sh` signs `aarch64_root/`, and `BOOTAA64.EFI` checks the manifest's signature and every image it loads (the services built for this platform) before loading. Evidence: every aarch64 suite and `aarch64_smoke.py` boot from signed volumes; the refusal cases run on x86 only. Slots A and B as on x86, tested by `aarch64_smoke.py` (a trial and its confirmation, an unconfirmed trial's fallback, a damaged slot, a torn record). The trust model and what is not met are x86's. |
| FP/SIMD state | not provided | Programs are built soft-float; `CPACR_EL1` traps FP/SIMD, and no FP state is saved. |
| Legacy devices | none | No port I/O, no ISA devices, no legacy VirtIO interface: `ata`, `ps2_kbd` and `audio_gw` are not built; `docs/legacy.md` lists nothing for this platform. |

## Not yet

- Boards on hardware: Raspberry Pi 4/5 with the EDK2 port (its xHCI is a VL805 behind a non-standard PCIe root), servers with ACPI; QEMU `sbsa-ref` (its firmware is not packaged) — issue [205](../../../issues/205-aarch64-boards.md). CPUs beyond eight, CPU hotplug.
- An SMMU, so the DMA drivers leave the TCB.
- FP/SIMD state, so programs are soft-float; sound (virtio-snd); `virtio_rng` for machines without RNDR.
- PAN (Privileged Access Never): not enabled yet; the kernel reaches task memory only through its identity map, never through user addresses.
- Apple Silicon Macs. Natively: issue [210](../../../issues/210-apple-silicon-native.md), track `APL` (no UEFI or ACPI, the AIC, a spin table, DARTs). In a virtual machine under the Mac's hypervisor (`-accel hvf -cpu host`, [docs/apple-silicon.md](../../apple-silicon.md)): not tested, no evidence (issue [600](../../../issues/600-apple-silicon-mac-vm-host.md)); the evidence of this profile does not carry over to it.

## Evidence

Which tests support which statement: [evidence.md](evidence.md).
