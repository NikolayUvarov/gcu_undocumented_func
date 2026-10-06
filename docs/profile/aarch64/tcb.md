# Trusted computing base — `aarch64/QEMU-virt-0`

Listed as differences from the [x86-64 TCB](../tcb.md); a guarantee not named here has the same TCB, with the x86 drivers replaced by those of this platform (`virtio_blk` for `ata`/`ahci`/`usb_storage`, `virtio_input` for `ps2_kbd`, no `audio_gw`).

"Kernel" includes the architecture layer `kernel/src/arch/aarch64/`: the board's layout read from ACPI (`board.rs`, `acpi.rs`), exception entry and the context switch (`context.rs`), the translation tables' descriptors and the identity map (`mmu.rs`), CPU start through PSCI and the trampoline (`cpu.rs`), the GICv3 distributor, redistributors, CPU interface and the ITS with its tables (`interrupts.rs`), the generic timer (`clock.rs`), ECAM configuration access and the MSI message (`pcicfg.rs`), the platform device table (`platform.rs`), the PL011 (`serial.rs`) and the ACPI tables it reads (`acpi.rs`).

| Guarantee | TCB on this platform |
|---|---|
| Memory isolation between tasks; kernel integrity | CPU (MMU, exception levels), the AAVMF firmware until ExitBootServices, the bootloader, the kernel, the Rust toolchain; **the DMA drivers and their devices: `virtio_blk`, `nvme`, `virtio_net`, `virtio_input`, `usb_host`** (no SMMU); `init`. The firmware's ACPI tables and memory map: the kernel trusts the MCFG's ECAM base, the MADT's MPIDRs (it starts CPUs there) and its GIC addresses (distributor, redistributors or CPU interface, ITS or GICv2m frame), the SPCR's UART, the GTDT's timer interrupt, the FADT's PSCI conduit, and the memory map's RAM (normal memory in its identity map of the first TiB, the rest device memory); addresses outside that TiB are refused. |
| Interrupt delivery | The GICv3 and its ITS, or a GICv2 with a GICv2m frame (issue 205); the kernel programs every MSI-X entry and every ITS mapping (`MAPD`, `MAPTI`), so a driver chooses neither the address nor the event of its device's messages. With the ITS a device can raise only the LPIs mapped for its requester ID; a GICv2m frame checks nothing: a device can raise any of the frame's SPIs. |
| CPU start, reset and power off | PSCI as implemented by the platform (QEMU, through HVC; firmware through SMC on hardware): `CPU_ON` starts a CPU at the kernel's trampoline, `SYSTEM_RESET` and `SYSTEM_OFF` end the machine. Only the holder of process control (the shell) may reset or power off. |
| Keyboard input reaches only the focused task | Kernel; `virtio_input`, `usb_hid` and `shell` (they hold the input privilege); `usb_host` (it hands `usb_hid` the keyboard's reports). |
| Screen shows the focused task | Kernel; `compositor`; the firmware's `ramfb` framebuffer as reported by GOP. |
| Calendar time | `rtc` with the PL031's registers (`PLATFORM_MMIO`), the only holder. |
| Secrecy of the device key; TLS key exchange | As on x86, with RNDR in place of RDRAND (no RNDR: no key and no TLS). |
| The console | The kernel's PL011 lines; the shell holds the PL011's registers once it runs (`PLATFORM_MMIO`) and then is the only writer besides the kernel's panic and boot lines. |

## Notes

- The three VirtIO drivers are the DMA gap of this platform, as the x86 storage, audio and network drivers are there; an SMMU (QEMU `iommu=smmuv3`, roadmap K2) would take them out.
- The ITS tables, the command queue and the redistributor's LPI tables are in the frame pool for the platform's lifetime ([kernel objects](../kernel-objects.md)); a device writing them by DMA could route interrupts, which is again the missing SMMU.
