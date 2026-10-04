# Threat and fault model — `x86-64/QEMU-0`

## Assets

1. Integrity of the kernel: its code, data, page tables and capability tables.
2. Memory isolation between tasks: no task reads or writes another task's private memory without a capability.
3. Capability confinement: a task can only use the authority it was granted.
4. Availability of the boot services and of the shell.
5. Integrity of programs started from the boot disk (limited, see below).

## Adversaries in scope

| Adversary | Can | Guarantee claimed |
|---|---|---|
| Malicious application | Any code in ring 3 with the standard client capabilities (RTC, VFS, audio, loader, TTS endpoints, its INIT slot) | Cannot read or write kernel memory or other tasks' private memory, cannot use privileged system calls, cannot obtain device access. Faults terminate only it. |
| Compromised service without DMA | Everything its capabilities allow (e.g. `rtc`: CMOS ports; `vfs_server`: block endpoints; `shell`: kill, focus, logs, input injection) | Damage is limited to those capabilities and what transitively reaches through them (MC-1.6, MC-3.9). |
| Malicious application and files | Writing files | An application's file client reads only; the shell's (the user's badge) writes on `ram:` and in `data/` of the boot disk, and the shell lends it to a program that asks for a file (`REQUEST_FILE`: the editor, the file manager) — that program can then change any file there, not only the one it was started with (issue 051); boot files and the rest of the disk are not writable through any client, and only `vfs_server` holds write-badged block clients (Appendix B.6). |
| Application keeping the file service busy | Asking `vfs_server` for checks or long operations in a loop | None beyond integrity: `vfs_server` serves one request at a time and a `check` reads the whole FAT and directory tree, so other clients wait; there is no per-client limit yet (MC-10.2). A check never changes the volume. |
| Service writing misleading or many log records | Text claiming another source; a flood of records | `logd` stamps each record with the sender's PID and task name from IPC and the kernel's task records (MC-10.6), so the source column cannot be forged — the text itself can say anything. At most 64 records a second per sender are kept; the rest are refused and counted, with a note in the log. The ring keeps 256 records: many senders together can push older records out, which readers see as gaps in the sequence numbers and in the dropped count. Only the shell's client may read. |
| Malicious file content | Crafted FAT structures and ELF files on the boot disk | FAT parsing happens in `vfs_server` (ring 3, block endpoints only). ELF images of applications are parsed by the kernel's ELF loader with bounds checks; the loader rejects malformed images. Program origin is **not** authenticated. |

## Out of scope (not claimed)

- **DMA-capable drivers and devices.** Without an IOMMU, `ahci`, `usb_storage`, `audio_gw` and the devices they program can access all physical memory. A compromise of any of them defeats every memory guarantee (MC-1.5).
- `init` and the platform privilege: `init` can mint device capabilities, DMA regions and privileges; it is trusted.
- Firmware, the UEFI bootloader, physical access, malicious hardware, supply chain of the toolchain.
- Side channels (caches, timing, speculative execution) and SMT interference.
- Denial of service by CPU consumption: there are no budgets; a busy task only shares its CPU round-robin.

## Fault model

| Fault | Behaviour |
|---|---|
| Exception in a user task | The task is terminated, the fault is recorded (`faults`), clients waiting for its reply get `ERR_PEER`, its memory is reclaimed once no other task maps it. |
| Task exits or is killed while focused | Focus returns to the focus owner (shell), which gets a notice. |
| Service dies | Its clients get `ERR_PEER`; new sends to its endpoint fail with `ERR_PEER`. It is restarted only by an explicit `RUN <service> &` (no automatic supervision). |
| Driver hangs | Not detected (no watchdog, no supervision). |
| Device misbehaves (DMA) | Not contained. |
| Kernel exception or panic | The system halts with a message on COM1. |
| Loss of the disk | Programs can no longer be loaded; running tasks continue. |
| Power loss or reset while files change | `vfs_server` keeps changed sectors in a write-back cache until a flush (`flush`, `sync`, the shell's file commands); everything since the last flush may be lost. A change is written as file data, then every FAT copy, then the directory entry, but the cache writes sectors in LBA order (the FAT before data), so a loss during a flush can leave: allocated clusters not yet in an entry (lost space), an entry with a size longer than its written data (stale bytes), or, after a move, the entry in both directories (a cross-link that `fsck` repairs). There is no journal and no atomicity beyond FAT itself; the dirty bit in FAT[1] (FAT16/32) marks a volume changed since the last flush, so a check knows to look. The RAM disk is lost at every reset by design. |
