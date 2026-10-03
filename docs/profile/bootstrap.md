# Bootstrap authority — `x86-64/QEMU-0`

MC-3.12 requires a verifiable boundary where the initial distribution of authority ends. In this profile:

1. The UEFI bootloader loads `kernel.elf` and the boot images listed in `BOOT_FILES` (`common/abi.rs`) and passes them to the kernel. It grants nothing.
2. The kernel starts exactly one task, boot image 0 (`init`), with:
   - slot 1: endpoint 10 (`EP_INIT`), all rights;
   - slot 2: the **platform** privilege (`PLATFORM_CAP`, `DEVICE_FIND`, spawning boot images and services);
   - slot 3: the **spawn** privilege;
   - the root quota: 19 tasks, 48 endpoints.
   Nothing else in the system holds the platform privilege unless `init` grants it (it does not).
3. `init` starts the other boot images in `BOOT_SERVICES` order. For each it mints the needed capabilities, passes them in the `SPAWN` grant list and drops its own copies, except DMA regions, which it keeps for driver restarts.

| Service | Receives from `init` |
|---|---|
| `rtc` | endpoint 2 (all), ports 0x70–0x71 |
| `ps2_kbd` | ports 0x60 and 0x64, IRQ 1, input privilege |
| `compositor` | framebuffer memory, display privilege |
| `ata` | endpoint 5 (all), ports 0x1F0–0x1F7 and 0x3F6 |
| `ahci` | endpoint 6 (all), ABAR (BAR5) MMIO of the first AHCI controller, 128 KiB DMA |
| `usb_storage` | endpoint 7 (all), BAR0 MMIO of the first xHCI controller, 256 KiB DMA |
| `vfs_server` | endpoint 3 (all), write/grant endpoints of the running block drivers |
| `loader` | endpoint 8 (all), write/grant endpoints 2, 3, 4, 9, spawn privilege |
| `audio_gw` | endpoint 4 (all); if an AC97 is present: its two port BARs, its IRQ, 132 KiB DMA |
| `tts` | endpoint 9 (all), write/grant endpoint 4 |
| `shell` | screen; write/grant endpoints 10 (init), 2, 3, 4, 8, 9; process-control and input privileges; ports 0x3F8–0x3FF |

4. Applications are started by `loader`, which grants the write/grant endpoints 2, 3, 4, 8, 9 and optionally an endpoint from the requesting program.

## Where initial distribution ends

The initial distribution is complete when `init` logs `[INIT] READY` (after starting `shell`). The boundary is **not** a reduction of authority: `init` keeps the platform and spawn privileges to restart services on request (`RUN <service> &`). This is recorded as a gap against the stage II exit criterion "boot authority is separated"; roadmap C6 replaces it with a supervisor that holds only what restarts need.

## Kernel-side validation

Whatever `init` asks for, the kernel only mints: reserved endpoints 1–15; port ranges inside 0x60, 0x64, 0x70–0x71, 0x1F0–0x1F7, 0x3F6, 0x3F8–0x3FF; IRQ lines 1–15 except 2; BARs and IRQ lines of enumerated PCI functions; the GOP framebuffer; DMA regions up to 8 MiB in total. PIC, PIT and PCI configuration ports are never handed out.
