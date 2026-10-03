# Bootstrap authority — `x86-64/QEMU-0`

MC-3.12 requires a verifiable boundary where the initial distribution of authority ends. In this profile:

1. The UEFI bootloader loads `kernel.elf` and the boot images listed in `BOOT_FILES` (`common/abi.rs`) and passes them to the kernel. It grants nothing.
2. The kernel starts exactly one task, boot image 0 (`init`), with:
   - slot 1: its own endpoint (created by the kernel, charged to init's quota), all rights;
   - slot 2: the **platform** privilege (`PLATFORM_CAP`, `DEVICE_FIND`, spawning boot images and services);
   - slot 3: the **spawn** privilege;
   - the root quota: 19 tasks, 63 endpoints.
   Nothing else in the system holds the platform privilege unless `init` grants it (it does not).
3. Endpoints have no numbers in the ABI. For each service `init` creates an endpoint with `ENDPOINT_CREATE` and keeps only a keeper capability (`CAP_KEEP | CAP_WRITE | CAP_GRANT`): it can mint a receive child for the server and write/grant children for clients but cannot receive itself, so a send to a service with no running server still fails with `ERR_PEER`. A restarted server receives a new child of the same endpoint.
4. `init` starts the other boot images in `BOOT_SERVICES` order. For each it mints the needed capabilities, moves them into the service through the `SPAWN` grant list (`GRANT_MOVE`), except DMA regions and its own endpoint, which it copies and keeps.

| Service | Receives from `init` |
|---|---|
| `rtc` | server endpoint, ports 0x70–0x71 |
| `ps2_kbd` | ports 0x60 and 0x64, IRQ 1, input privilege |
| `compositor` | framebuffer memory, display privilege |
| `ata` | server endpoint, ports 0x1F0–0x1F7 and 0x3F6 |
| `ahci` | server endpoint, ABAR (BAR5) MMIO of the first AHCI controller, 128 KiB DMA |
| `usb_storage` | server endpoint, BAR0 MMIO of the first xHCI controller, 256 KiB DMA |
| `vfs_server` | server endpoint, client endpoints of the running block drivers |
| `loader` | server endpoint, client endpoints of `rtc`, `vfs_server`, `audio_gw`, `tts`, spawn privilege |
| `audio_gw` | server endpoint; if an AC97 is present: its two port BARs, its IRQ, 132 KiB DMA |
| `tts` | server endpoint, client endpoint of `audio_gw` |
| `shell` | screen; client endpoints of `init`, `rtc`, `vfs_server`, `audio_gw`, `loader`, `tts`; process-control and input privileges; ports 0x3F8–0x3FF |

5. Applications are started by `loader`, which grants its client endpoints of `rtc`, `vfs_server`, `audio_gw`, `tts` and optionally an endpoint from the requesting program.

## Where initial distribution ends

The initial distribution is complete when `init` logs `[INIT] READY` (after starting `shell`). The boundary is **not** a reduction of authority: `init` keeps the platform and spawn privileges to restart services on request (`RUN <service> &`). This is recorded as a gap against the stage II exit criterion "boot authority is separated"; roadmap C6 replaces it with a supervisor that holds only what restarts need.

## Kernel-side validation

Whatever `init` asks for, the kernel only mints: port ranges inside 0x60, 0x64, 0x70–0x71, 0x1F0–0x1F7, 0x3F6, 0x3F8–0x3FF; IRQ lines 1–15 except 2; BARs and IRQ lines of enumerated PCI functions; the GOP framebuffer; DMA regions up to 8 MiB in total. PIC, PIT and PCI configuration ports are never handed out.
