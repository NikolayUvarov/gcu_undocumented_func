# Threat and fault model — `aarch64/QEMU-virt-0`

The assets, adversaries in scope, the out-of-scope list and the fault model are those of [x86-64/QEMU-0](../threat-model.md), with the differences below.

## Differences

| Topic | On this platform |
|---|---|
| DMA-capable drivers | `virtio_blk`, `virtio_net`, `virtio_input` and their devices, without an SMMU: out of scope, as on x86 (MC-1.5). |
| Interrupts from devices | MSIs go through the ITS, which checks the device's requester ID: a device cannot raise another device's LPIs by writing the translation register (no such check on x86). Wired lines are SPIs a driver gets only through `init`. |
| Legacy devices | None: no I/O ports, no ISA devices; the `PLATFORM_PORTS` table is empty and `PLATFORM_MMIO` lists only the PL011 and PL031. |
| Firmware tables | The ACPI MCFG, MADT and FADT are trusted as given, as the FADT is on x86; an ECAM above 4 GiB is refused (no PCI), a MADT with more than 256 CPUs is cut to 256 (issue 171). |
| Malicious application | As on x86, at EL0: every synchronous exception from EL0 ends only the task (`aarch64_smoke.py`); kernel memory is EL1-only in the translation tables (no PAN: the kernel never dereferences user addresses directly, it translates them through the task's tables). |
| FP/SIMD | Not available to programs (trapped); no state can leak between tasks through it. |

## Fault model: differences

| Fault | Behaviour |
|---|---|
| A CPU that does not start | Reported (`A CPU DID NOT COME ONLINE`, or `PSCI CPU_ON REFUSED`); the system runs on the others. |
| The firmware stalls before the bootloader | Not ours to contain; the smoke test boots once more and says so. |
| Kernel exception or panic | The system halts with a message on the PL011. |
| Reset or power off | PSCI; if the call returns, the CPUs halt. |
