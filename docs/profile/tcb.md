# Trusted computing base — `x86-64/QEMU-0`

The TCB is listed per guarantee (MC-1.6, MC-12.1). "Kernel" is everything in `kernel/` plus `common/abi.rs`; `bootloader/src/elf_reloc.rs` is also compiled into the kernel.

| Guarantee | TCB |
|---|---|
| Memory isolation between tasks | CPU (MMU, privilege levels), UEFI firmware until ExitBootServices, bootloader, kernel, Rust toolchain; **every DMA-capable driver and its device: `ahci`, `usb_storage`, `audio_gw`** (no IOMMU); `init` (it can mint DMA regions and device MMIO capabilities). |
| Kernel integrity | Same as above. |
| Capability confinement of applications | Kernel; `loader` (decides which client capabilities an application gets); `init` (decides what `loader` holds). |
| Capability confinement of services | Kernel; `init`. |
| Correct program images | Boot disk contents (not authenticated), `ata`/`ahci`/`usb_storage`, `vfs_server`, `loader`, kernel ELF loader. |
| Process control (kill, focus, logs) used only as the user intends | Kernel; `init` (grants the control privilege); `shell`. |
| Keyboard input reaches only the focused task | Kernel; `ps2_kbd` and `shell` (both hold the input privilege and can inject arbitrary input). |
| Screen shows the focused task | Kernel; `compositor` (display privilege, framebuffer). |

## Notes

- A driver running in ring 3 is not automatically outside the TCB: its capabilities decide that. The three DMA drivers are the main gap and are tracked as roadmap K2 (VT-d) / III-4.
- `init` is in the TCB of nearly everything by design: it is the bootstrap authority. Narrowing it (a minimal supervisor that keeps only what restarts need) is part of roadmap C6.
- The shell's control privilege is broad (kill any task, read any log). Splitting it is future work.
