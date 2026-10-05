# Platform profile `aarch64/QEMU-virt` (draft, issue 201)

**Version:** 0.1 (2026-10-05) · **Roadmap:** track H · **Status:** boots to init with the services that need no devices; not a full platform yet (202–204).

This is the second platform of MIND Core. It shares the kernel's generic part, every service's source and the system-call interface with `x86-64/QEMU-0` ([README.md](README.md)); what differs is the architecture layer (`kernel/src/arch/aarch64/`, `libmind/src/arch/aarch64.rs`) and the bootloader's few architecture lines.

## Machine

QEMU `virt` with `gic-version=3`, `-cpu max`, 512 MiB, AAVMF (EDK2) firmware, a `ramfb` display for the firmware's graphics output. The platform layout is fixed in the architecture layer until the device tree or ACPI is read (202): GICv3 distributor at `0x0800_0000`, CPU 0's redistributor at `0x080A_0000`, PL011 at `0x0900_0000`, RAM from 1 GiB.

## What the kernel does here

| Mechanism | aarch64 | x86-64 counterpart |
|---|---|---|
| Privilege | Tasks at EL0, the kernel at EL1 | ring 3, ring 0 |
| System call | `svc #0`, the same mailbox ABI | `int 0x80` |
| Exceptions | EL1 vector table (`VBAR_EL1`); a per-CPU exception stack (`TPIDR_EL1`) for entries from tasks | IDT, TSS stacks |
| Address spaces | Stage 1, 4 KiB granule, 4 levels, 48-bit TTBR0: L0 entry 0 the kernel's identity map of 4 GiB (1 GiB blocks, EL1 only), the user window like on x86; user pages `PXN`, data pages `UXN`, code read-only; the generic walk is `kernel/src/paging.rs` | 4-level page tables, NX |
| Interrupts | GICv3: SPI 32 + n is device line n, SGIs 1–3 for stop, tick and wake (used from 203) | 8259 PIC, xAPIC, MSI-X |
| Tick and clock | EL1 virtual timer, 100 Hz; `CNTVCT_EL0` at `CNTFRQ_EL0` for the monotonic clock | PIT, TSC |
| FP/SIMD | Disabled (`CPACR_EL1`): programs and kernel are soft-float, no FP state is saved yet | x87/SSE/AVX saved per task |
| Entropy | `RNDR` when `ID_AA64ISAR0_EL1` lists it; the kernel tells tasks in `BootInfo.cpu_features` (EL0 cannot read ID registers) | `RDRAND` |
| Reset | PSCI `SYSTEM_RESET` (HVC) | ACPI reset register, 0xCF9, 8042 |
| Console | PL011; every task's log is also written there, since no shell runs yet | COM1, the shell |
| Instruction cache | Invalidated after the kernel writes program images and the exit page | coherent |

## Programs

The same sources, built for `aarch64-unknown-none-softfloat` as static PIEs with 4 KiB segments (`.cargo/config.toml` of each program); the loaders accept `R_AARCH64_RELATIVE`. `scripts/build_aarch64.sh` builds the bootloader, the kernel and `init`, `logd`, `loader`, `sysmon`, `keystore`; the bootloader leaves out the boot images it does not find, and init reports them as failed.

## Not yet

- Devices: PCIe ECAM, VirtIO, the PL011 for the shell, PL031, display (issue 202); then the shell and the applications.
- More CPUs, power off (issue 203); a profile with CI on hardware (204).
- PAN (Privileged Access Never): not enabled yet; the kernel reaches task memory only through its identity map, never through user addresses.

## Evidence

`tests/aarch64_smoke.py` (CI job `aarch64`): the boot reaches `[INIT] READY` with `logd`, `loader`, `keystore` (device key from RNDR) and `sysmon`; a service that reads kernel memory, writes its code, executes its stack or runs an undefined instruction is ended with that exception class (`FAULT VECTOR` 36, 36, 32, 0), restarted by init and quarantined after three restarts, while the others keep running.
