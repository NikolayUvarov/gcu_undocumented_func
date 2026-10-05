# Trusted computing base — `x86-64/QEMU-0`

The TCB is listed per guarantee (MC-1.6, MC-12.1). "Kernel" is everything in `kernel/` plus `common/abi.rs`; `bootloader/src/elf_reloc.rs` is also compiled into the kernel. The kernel includes its **architecture layer**, `kernel/src/arch/x86_64/` (issue 200; `arch/aarch64/` on the second platform, [aarch64-qemu-virt.md](aarch64-qemu-virt.md)): interrupt entry and the context switch, page tables, CPU start and stop, the APIC, PIC and PIT, the clock, port I/O, PCI configuration access, the serial line and ACPI reset. The rest of the kernel reaches the processor only through it. In every task, `libmind/src/arch/` (the system-call and entropy instructions) is part of that program, not of the TCB, except where the program itself is in the TCB (`keystore`: RDRAND for the device key).

| Guarantee | TCB |
|---|---|
| Memory isolation between tasks | CPU (MMU, privilege levels), UEFI firmware until ExitBootServices, bootloader, kernel, Rust toolchain; **every DMA-capable driver and its device: `ahci`, `usb_storage`, `audio_gw`, `virtio_net`** (no IOMMU); `init` (it can mint DMA regions and device MMIO capabilities). |
| Kernel integrity | Same as above. |
| Capability confinement of applications | Kernel; `loader` (decides which client capabilities an application gets); `init` (decides what `loader` holds). |
| Capability confinement of services | Kernel; `init`. |
| Correct program images | Boot disk contents (not authenticated), `ata`/`ahci`/`usb_storage`, `vfs_server`, `loader`, kernel ELF loader. |
| Integrity of disk contents (writes) | Kernel (badges); `init` (gives the write-badged block clients to `vfs_server` only); `ata`/`ahci`/`usb_storage` (check the badge, write the medium); `vfs_server` (the only writer; it confines the user's client to `ram:` and `data/`); `shell` (holds the user's file client). |
| Process control (kill, focus, logs) used only as the user intends | Kernel; `init` (grants the control privilege); `shell`. |
| Keyboard input reaches only the focused task | Kernel; `ps2_kbd`, `virtio_input` and `shell` (all hold the input privilege and can inject arbitrary input). |
| Screen shows the focused task | Kernel; `compositor` (display privilege, framebuffer). |

## Notes

- A driver running in ring 3 is not automatically outside the TCB: its capabilities decide that. The three DMA drivers are the main gap and are tracked as roadmap K2 (VT-d) / III-4.
- `init` is in the TCB of nearly everything by design: it is the bootstrap authority. Narrowing it (a minimal supervisor that keeps only what restarts need) is part of roadmap C6.
- The shell's control privilege is broad (kill any task, read any log). Splitting it is future work.
| Secrecy of the device key (issue 103) | CPU (RDRAND), kernel (memory isolation, above); `keystore` (makes and holds the key, the only process that can read it); `init` (gives the signer's badge to `tls` only). |
| TLS sessions: server authentication, confidentiality and integrity | `tls` with rustls, rustls-webpki and the RustCrypto crates of its provider; RDRAND (key exchange); `rtc` (certificate validity times); the root store `tlsroots.pem` on the boot disk, which is not authenticated (Article 9), and `vfs_server`, which reads it; `keystore` for client authentication. `netstack`, `virtio_net` and the network see only TLS records. |
| Availability after `REBOOT` | Firmware (reset and boot), the ACPI tables (the FADT reset register is trusted as given), kernel `arch/x86_64/acpi.rs`; only the holder of process control (the shell) may reset. |
