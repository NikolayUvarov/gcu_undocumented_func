# Bootstrap authority — `x86-64/QEMU-0`

MC-3.12 requires a verifiable boundary where the initial distribution of authority ends. In this profile:

1. The UEFI bootloader loads `kernel.elf` and the boot images listed in `BOOT_FILES` (`common/abi.rs`) and passes them to the kernel. It grants nothing.
2. The kernel starts exactly one task, boot image 0 (`init`), with:
   - slot 1: its own endpoint (created by the kernel, charged to init's quota), all rights;
   - slot 2: the **platform** privilege (`PLATFORM_CAP`, `DEVICE_FIND`, spawning boot images and services);
   - slot 3: the **spawn** privilege;
   - the root quota: 31 tasks, 127 endpoints.
   Nothing else in the system holds the platform privilege unless `init` grants it (it does not).
3. Endpoints have no numbers in the ABI. For each service `init` creates an endpoint with `ENDPOINT_CREATE` and keeps only a keeper capability (`CAP_KEEP | CAP_WRITE | CAP_GRANT`): it can mint a receive child for the server and write/grant children for clients but cannot receive itself, so a send to a service with no running server still fails with `ERR_PEER`. A restarted server receives a new child of the same endpoint.
4. `init` starts the other boot images in `BOOT_SERVICES` order, `logd` first. For each it mints the needed capabilities and moves them into the service through the `SPAWN` grant list (`GRANT_MOVE`); plain client endpoints (no badge), DMA regions and its own endpoint it copies from what it keeps, narrowed to the rights the service gets, so it needs no slot of its own for them. Every service except `logd` gets a `logd` client in slot 12; `init` itself writes to `logd` through its keeper once `logd` runs (its earlier lines are kept and sent then).

| Service | Receives from `init` |
|---|---|
| `logd` | server endpoint, observe privilege (to name the sender of a record from the kernel's task records) |
| `rtc` | server endpoint, ports 0x70–0x71 |
| `ps2_kbd` | server endpoint (requests from the shell's keyboard client, issue 085), ports 0x60 and 0x64, IRQ 1, input privilege |
| `compositor` | server endpoint (requests from the shell's display client, issue 086), framebuffer memory, display privilege |
| `ata` | server endpoint, ports 0x1F0–0x1F7 and 0x3F6 |
| `ahci` | server endpoint, ABAR (BAR5) MMIO of the first AHCI controller, 128 KiB DMA |
| `usb_storage` | server endpoint, BAR0 MMIO of the first xHCI controller, 256 KiB DMA |
| `ramdisk` | server endpoint |
| `vfs_server` | server endpoint, write-badged client endpoints of the running block drivers and of `ramdisk`, an `rtc` client (slot 6) |
| `loader` | server endpoint, client endpoints of `rtc`, `vfs_server`, `audio_gw`, `tts`, spawn privilege |
| `audio_gw` | server endpoint; if an AC97 is present: its two port BARs, its IRQ, 132 KiB DMA |
| `tts` | server endpoint, client endpoint of `audio_gw` |
| `virtio_net` | server endpoint; if a VirtIO network card (1AF4:1041 or 1000) is present: the memory BAR holding its modern configuration structures and an MSI-X vector (the IRQ line if MSI-X cannot be set up), or for a legacy-only card its I/O BAR0 and IRQ line; 160 KiB DMA |
| `netstack` | server endpoint, a client of `virtio_net` (slot 2) |
| `sysmon` | server endpoint, observe privilege (read-only statistics; only `sysmon` and `logd` get it) |
| `shell` | screen; client endpoints of `init`, `rtc`, `vfs_server` (with the user's badge: writes on `ram:` and in `data/`), `audio_gw`, `loader`, `tts`, `sysmon` (slot 10), `logd` with the read badge (slot 12), `sysmon` with the authority badge (slot 13), `ps2_kbd` (slot 14), `compositor` (slot 15), `virtio_net` (slot 16, the `net` diagnostics), `netstack` (slot 17); process-control and input privileges; ports 0x3F8–0x3FF |

5. Applications are started by `loader`, which grants its client endpoints of `rtc`, `vfs_server`, `audio_gw`, `tts` and optionally an endpoint from the requesting program. In a launch session (`idl/loader.wit`) the launcher lends further capabilities for slots 7–12 — the shell lends its `sysmon` client (slot 10) to a program whose `.mind_request` section asks for it, a VFS client confined to the directory of the file it is started with (slot 7, made by `vfs_server`'s `scope` from the shell's own handle, never wider) to one that asks for a file (`REQUEST_FILE`, the editor), its own VFS client (slot 7, the user's badge) to one that asks for the user's files (`REQUEST_FILES`, the file manager), and its read-badged `logd` client (slot 12) to one that asks for the log (`REQUEST_LOG`, `dmesg`), and its client of `init` (slot 11) to one that asks for lifecycle control (`REQUEST_LIFECYCLE`: `svc`, `top`); a program that asks for the console starts without a screen.

## Where initial distribution ends

The initial distribution ends when `init` logs `[INIT] PLATFORM PRIVILEGE DROPPED` and `[INIT] READY` (after starting `shell`). Before that, `init` mints a **restart** privilege (it can only spawn boot images and services) and drops the platform privilege (slot 2). From then on:
- `init` restarts services only from the capabilities it handed out at the first start, which it keeps (copies go to each instance);
- it stops and restarts their devices through the BAR capabilities it holds (`DEVICE_STATE`);
- a service whose hardware was missing at boot cannot be started later.

`init` remains the supervisor (roadmap C6, issue 032). If it ends, the kernel halts the system (MC-6.8).

## Kernel-side validation

Whatever `init` asks for, the kernel only mints: port ranges inside 0x60, 0x64, 0x70–0x71, 0x1F0–0x1F7, 0x3F6, 0x3F8–0x3FF; IRQ lines 1–15 except 2; BARs and IRQ lines of enumerated PCI functions; the GOP framebuffer; DMA regions up to 8 MiB in total. PIC, PIT and PCI configuration ports are never handed out.
