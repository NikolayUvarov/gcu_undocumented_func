# 174-KRN-0038 — A complete hardware report on every boot

**Type:** kernel · **Owner:** `KRN` · **Priority:** P1 · **Status:** in progress (done in QEMU; the MacBook Pro's run left) · **Blocked by:** — · **Main task:** [174](174-full-use-of-pc-hardware.md) · **Constitution:** MC-1.2, MC-10.2, MC-10.4, MC-12.1

## Problem

The maintainer asked on 2026-10-09 that the system, at every start, save everything it can learn about the hardware into a separate file, complete and exhaustive, so that software can be debugged and the system developed with it. A GPU or NPU present must be known in full. Today nothing records the machine:

- what the kernel learns (CPUID, ACPI tables, PCI functions, its own decisions) it prints to COM1, and to the screen only until the compositor takes it, so on a machine without COM1 most of it is lost (the MacBook Pro, 211-KRN-0021);
- the drivers' lines (disk models, USB devices) are in the system log, mixed with everything else.

## Plan

The kernel builds the report, since it alone sees the CPU's model-specific registers, the firmware's tables and every function's configuration space. init writes it, since it holds the file system at boot.

- **Kernel lines kept.** Every kernel line, those after the compositor took the screen included, also goes into a ring of the last 32 KiB. The report carries it.
- **`PLATFORM_CAP` kinds** (platform privilege, so init only, during boot):
  - `PLATFORM_REPORT`: a read-only memory object with the report's text, built when asked;
  - `PLATFORM_ACPI_TABLE` with an index: a read-only copy of that ACPI table.
- **The report** (text, one section per subject):
  - **CPU:**
    - vendor, brand and family, model and stepping;
    - microcode revision;
    - every CPUID leaf and subleaf as raw words, then decoded: feature names, caches (level, type, size, ways, line, sharing), XSAVE components with sizes, the TSC and nominal frequencies;
    - the CPUs' APIC IDs;
    - the model-specific registers that describe performance (platform info, turbo ratios, HWP capabilities, AMD P-states), each read so that a missing one is reported, not fatal.
  - **Memory:** the firmware's memory map, every range with its type, and the totals.
  - **Firmware:** the RSDP, and every ACPI table with its signature, length, revision and OEM IDs. Decoded:
    - the MADT (CPUs, I/O APICs, overrides);
    - the FADT (reset, PM timer, flags);
    - the MCFG;
    - whether DMAR or IVRS (an IOMMU), SRAT or SLIT (NUMA), and HPET are present.
  - **PCI:** every function with:
    - location, vendor and device, subsystem, class with its name, revision;
    - BARs (address, size, 32 or 64 bits, prefetchable or I/O);
    - command and status, interrupt line and pin;
    - capabilities: power management, MSI, MSI-X (table size), PCI Express (port type, link speed and width, both maximum and current, payload);
    - the 256 bytes of configuration space in hex.
  - **GPUs and NPUs** (classes 03, 12 and 0B40) marked as such, with what their registers say where it is safe to read them (NVIDIA: the chip's ID from `PMC_BOOT_0`).
  - **The kernel's decisions:** the tick source and TSC frequency, CPUs started, x2APIC, the saved vector state, and the kernel ring.
- **Files.** When the services have started, init writes the report to `log:hwNNNN.txt` (`NNNN` of this boot's `bootNNNN.log`), and each ACPI table to `log:acpi/<SIG>.bin` (`SSDT1.bin`, `SSDT2.bin`, … for repeated signatures). Without a log volume the report goes to `ram:hardware.txt`.
- **Authority and privacy.** Only init gets the report, with the platform privilege at boot (MC-10.2: no new path to memory). The report holds identifiers (MAC addresses, serial numbers), stays on the machine's log volume, and leaves it only when the user copies it.
- **aarch64:** the same sections from the ID registers (MIDR, MPIDR, ID_AA64*), the GIC and timer, and the ACPI tables or the device tree.
- **Later steps of 174** add to the report what they learn: SMBIOS (needs the bootloader to pass its entry point), the drivers' devices (NVMe identify, USB descriptors, EDID), and NUMA nodes.

## Acceptance criteria

- **QEMU, x86 and aarch64:** the report holds every section, the CPU's brand and every PCI function `devices` lists, and the ACPI tables as files that match the firmware's. A machine with and without a log volume is covered.
- **The MacBook Pro:** the report and `log:acpi/` are on the USB drive after a boot. Its PCI section names both GPUs and both EHCI controllers with their BARs, and its kernel ring shows the lines the screen lost.

## Progress

**2026-10-09: written at every boot, x86 and aarch64.**

- **Kernel.**
  - `kernel/src/klog.rs` keeps the kernel's last 32 KiB of lines: every `serial_print`, so also what the screen lost.
  - `kernel/src/report.rs` builds the text: the CPU part (`arch/*/report.rs`), the firmware's memory map, every ACPI table decoded, every PCI function, the kernel's choices and the ring.
    - The ACPI decoding covers the MADT, FADT, MCFG, HPET, DMAR and IVRS (an IOMMU), SRAT and SLIT (NUMA), TPM2 and BGRT.
    - Each PCI function gets its BARs, bridge windows, capabilities (MSI, MSI-X, PCI Express link, power management) and 256 bytes of configuration space; GPUs and NPUs are marked, and an NVIDIA GPU's `PMC_BOOT_0` is read.
  - `PLATFORM_REPORT` and `PLATFORM_ACPI_TABLE` give read-only copies.
- **x86 CPU section.**
  - Every CPUID leaf and subleaf raw, then decoded:
    - feature names as Linux spells them;
    - caches, from leaf 4, AMD's 0x8000_001D, or 0x8000_0005/6;
    - topology, and the core type on hybrid CPUs;
    - XSAVE components with sizes and offsets;
    - TSC and nominal frequencies.
  - Microcode, and the model-specific registers for frequency and power (Intel: platform info, turbo ratios, HWP, RAPL, thermal; AMD: P-states, CPPC). `rdmsr` runs with a fault fixup, so a register the CPU lacks reads as "absent".
- **aarch64 CPU section.**
  - MIDR, MPIDR, the ID_AA64 registers raw and decoded: FP, SIMD, SVE, SME, MTE, BTI, pointer authentication, crypto, RNDR, address size, granules, PAN, VHE.
  - Caches from CLIDR and CCSIDR, and the timer's frequency.
- **init** (`init/src/hardware.rs`), before it drops the platform privilege:
  - writes `log:hwNNNN.txt`, numbered after this boot's `bootNNNN.log`, and keeps the last 50 as the boot logs are kept;
  - writes every table to `log:acpi/<SIG>.bin` (RSDP and the DSDT included; SSDTs numbered);
  - without a log volume, writes `ram:hardware.txt`;
  - logs `[INIT] HARDWARE REPORT: …`.
- **Tests.**
  - The `normal` suite (x86 and aarch64) finds `ram:hardware.txt` with the report's header, the CPU section and the started CPUs.
  - `usb_image_smoke.py` reads `HWNNNN.TXT` and `ACPI/*.bin` from the log partition on the host. It checks every section and the kernel's own lines, and that FACP, DSDT and APIC each have their signature and length, and the RSDP its signature.
- **Docs:** `docs/api` (the two kinds), the disk-writing guide (section 9, EN and RU), the log volume's README.

Left:
- the MacBook Pro's report;
- the later steps of 174: SMBIOS (the bootloader to pass its entry point), the drivers' devices (NVMe identify, USB descriptors, HDA codecs, EDID), and NUMA nodes.

## Related

[174](174-full-use-of-pc-hardware.md), [211-KRN-0019](211-KRN-0019-boot-logs-on-the-log-partition.md) (the log partition), [211-KRN-0021](211-KRN-0021-registers-inside-a-page.md).
