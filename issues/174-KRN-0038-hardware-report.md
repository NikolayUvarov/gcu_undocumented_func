# 174-KRN-0038 — A complete hardware report on every boot

**Type:** kernel · **Owner:** `KRN` · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [174](174-full-use-of-pc-hardware.md) · **Constitution:** MC-1.2, MC-10.2, MC-10.4, MC-12.1

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

## Related

[174](174-full-use-of-pc-hardware.md), [211-KRN-0019](211-KRN-0019-boot-logs-on-the-log-partition.md) (the log partition), [211-KRN-0021](211-KRN-0021-registers-inside-a-page.md).
