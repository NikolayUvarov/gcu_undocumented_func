# Platform profile `aarch64/QEMU-virt` (draft, issues 201–202)

**Version:** 0.2 (2026-10-05) · **Roadmap:** track H · **Status:** the system with its devices: shell, files, network, TLS, display and input on one CPU; more CPUs and the profile's CI are 203–204.

This is the second platform of MIND Core. It shares the kernel's generic part, every service's source and the system-call interface with `x86-64/QEMU-0` ([README.md](README.md)); what differs is the architecture layer (`kernel/src/arch/aarch64/`, `libmind/src/arch/aarch64.rs`) and the bootloader's few architecture lines.

## Machine

QEMU `virt,gic-version=3,highmem=off`, `-cpu max`, 512 MiB, AAVMF (EDK2) firmware, a `ramfb` display for the firmware's graphics output, and VirtIO PCI devices: the boot disk on `virtio-blk`, `virtio-net`, `virtio-keyboard` and `virtio-tablet`. `highmem=off` keeps the PCIe ECAM below 4 GiB, inside the kernel's identity map; with it above, the kernel reports `PCI: ECAM ABOVE 4 GIB, NOT USED` and runs without PCI.

The kernel reads the ECAM's place from the ACPI MCFG. The rest of the layout is fixed in the architecture layer: GICv3 distributor at `0x0800_0000`, its ITS at `0x0808_0000`, CPU 0's redistributor at `0x080A_0000`, PL011 at `0x0900_0000` (SPI 1), PL031 at `0x0901_0000` (SPI 2), RAM from 1 GiB.

## What the kernel does here

| Mechanism | aarch64 | x86-64 counterpart |
|---|---|---|
| Privilege | Tasks at EL0, the kernel at EL1 | ring 3, ring 0 |
| System call | `svc #0`, the same mailbox ABI | `int 0x80` |
| Exceptions | EL1 vector table (`VBAR_EL1`); a per-CPU exception stack (`TPIDR_EL1`) for entries from tasks | IDT, TSS stacks |
| Address spaces | Stage 1, 4 KiB granule, 4 levels, 48-bit TTBR0: L0 entry 0 the kernel's identity map of 4 GiB (1 GiB blocks, EL1 only), the user window like on x86; user pages `PXN`, data pages `UXN`, code read-only; the generic walk is `kernel/src/paging.rs` | 4-level page tables, NX |
| Interrupts | GICv3: SPI 32 + n is device line n (PCI INTA–D of slot s, pin p: line 3 + (s + p − 1) mod 4); MSI-X through the ITS, line 16 + n is LPI 8192 + n (event n of the device's requester ID, collection 0 on CPU 0); SGIs 1–3 for stop, tick and wake (used from 203) | 8259 PIC, xAPIC, MSI-X |
| PCI | ECAM from the MCFG, the same enumeration, BARs and MSI-X tables (`kernel/src/pci.rs`; the architecture gives configuration access, the legacy line and the MSI message) | ports `0xCF8`/`0xCFC` |
| Platform devices | `PLATFORM_MMIO` hands out exactly the PL011 and PL031 registers, `PLATFORM_IRQ` their lines 1 and 2 (`arch/aarch64/platform.rs`); there are no I/O ports | ISA port ranges and lines 1–15 |
| Tick and clock | EL1 virtual timer, 100 Hz; `CNTVCT_EL0` at `CNTFRQ_EL0` for the monotonic clock | PIT, TSC |
| FP/SIMD | Disabled (`CPACR_EL1`): programs and kernel are soft-float, no FP state is saved yet | x87/SSE/AVX saved per task |
| Entropy | `RNDR` when `ID_AA64ISAR0_EL1` lists it; the kernel tells tasks in `BootInfo.cpu_features` (EL0 cannot read ID registers) | `RDRAND` |
| Reset | PSCI `SYSTEM_RESET` (HVC) | ACPI reset register, 0xCF9, 8042 |
| Console | PL011: the kernel's lines, and every task's log until the shell gets the PL011 (`mind::dev::Uart`), then the shell's console as on COM1 | COM1, the shell |
| Instruction cache | Invalidated after the kernel writes program images and the exit page | coherent |

## Programs

The same sources, built for `aarch64-unknown-none-softfloat` as static PIEs with 4 KiB segments (`.cargo/config.toml` of each program); the loaders accept `R_AARCH64_RELATIVE`. `scripts/build_aarch64.sh` builds the bootloader and every crate of `02_build.sh` except the ISA drivers (`ata`, `ps2_kbd`, `audio_gw`), into `aarch64_root/`; `virtio_net` without its legacy (port I/O) interface. The bootloader leaves out the boot images it does not find, and init reports them as `NOT STARTED: NO IMAGE`.

## Devices and their services

| Device | Service | Notes |
|---|---|---|
| virtio-blk (PCI) | `virtio_blk` (both architectures) | the boot disk; modern interface, requests polled; vfs mounts FAT from it (`FROM VIRTIO`) |
| virtio-net (PCI) | `virtio_net` | modern interface, MSI-X through the ITS (INTx without it) |
| virtio-keyboard, virtio-tablet (PCI) | `virtio_input` | up to two devices; keys go through the PS/2 decoder (layouts, Ctrl+Z, the keyboard service `idl/keyboard.wit`), the tablet gives absolute pointer events |
| PL011 | `shell` | its console, as COM1 on x86 |
| PL031 | `rtc` | seconds since 1970, UTC |
| ramfb | `compositor` | the firmware's GOP framebuffer (800x600) |

## Not yet

- More CPUs, power off (issue 203); a profile with CI on hardware (204).
- FP/SIMD state, so programs are soft-float; sound (virtio-snd); `virtio_rng` for machines without RNDR.
- The ECAM above 4 GiB (`highmem=on`): the kernel would need to map it.
- PAN (Privileged Access Never): not enabled yet; the kernel reaches task memory only through its identity map, never through user addresses.

## Evidence

`tests/qemu_smoke.py --arch aarch64 --suites normal,shell,vfs,net,tls` (CI job `aarch64`) runs the x86 suites on `virt`: programs and the shell (instances, foreground, Ctrl+Z from the VirtIO keyboard, limits, heap baseline, the idle CPU waiting in WFI), line editing and history from both keyboards, files on a raw FAT disk through `virtio_blk` (fsck.fat, mtools, reboot through PSCI), the network (DHCP, DNS, TCP, flow grants, two cards, driver restarts, MSI-X through the ITS) and TLS (with RNDR, and fail-closed on a Cortex-A72 without it).

`tests/aarch64_smoke.py` (same job), without the shell and without PCI: the boot reaches `[INIT] READY` with `logd`, `loader`, `keystore` (device key from RNDR) and `sysmon`; a service that reads kernel memory, writes its code, executes its stack or runs an undefined instruction is ended with that exception class (`FAULT VECTOR` 36, 36, 32, 0), restarted by init and quarantined after three restarts, while the others keep running.
