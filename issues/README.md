# issues/ — working tasks

## Rules

1. **`issues/` holds only the tasks that are being worked on or scheduled for the near term.** Long-term direction lives in [ROADMAP.md](../ROADMAP.md); a roadmap item becomes an issue here when work on it is about to start.
2. **A finished task leaves this directory.** When its acceptance criteria are met, move it with `git mv` to [`issues-done/`](../issues-done/) and change the extension to `.done`, in the same commit as the work (or right after it). In the moved file:
   - append ` — done` to the title and set `Status: done (YYYY-MM-DD)`;
   - add a `## Resolution` (or `## Done`) section saying what was done and where (files, tests);
   - fix relative links (`../issues/…md` for open tasks, `….done` for finished ones).
3. **A task that became irrelevant** (replaced by another design or by a different task) is moved the same way with status `superseded` and a resolution naming what replaced it. Unfinished remainders are split into a new issue rather than keeping the old one open.
4. **One file per task**: `NNN-short-name.md`, numbers are never reused. Format: title, metadata line (Type · Priority · Status · Blocked by), Problem, Plan, Acceptance criteria, Related. Every task names the Constitution articles or roadmap item it serves.
5. Update the tables below in the same commit.

## Open tasks

| № | Task | Type | Priority | Blocked by | Roadmap |
|---|---|---|---|---|---|
| [009](009-gop-pixel-format.md) | Honour `PixelFormat` and select a GOP mode | bug/robustness | medium | — | Stage I |
| [011](011-reproducible-toolchain.md) | Reproducible build: `rust-toolchain.toml`, `Cargo.lock`, workspace | infra | low | — | Track C (can start now) |
| [018](018-kernel-panic-diagnostics.md) | Kernel panic prints message and location | robustness | medium | — | Assurance |
| [050](050-svc-lifecycle.md) | `svc` and lifecycle control | feature | P1 | 038; C6 | G, C6 |
| [051](051-scoped-file-grants.md) | Scoped file grants for launched programs | security/architecture | P1 | — | G (tools F7) |

## Finished tasks (`issues-done/`)

| № | Task | Result |
|---|---|---|
| [001](../issues-done/001-flat-binary-entry-offset-and-got-call.done) | Flat binaries: `_start` offset and GOT call | superseded by 002 (2026-10-03) |
| [002](../issues-done/002-elf-loader.done) | ELF loader instead of flat binaries | done (2026-10-03) |
| [003](../issues-done/003-bss-and-heap-allocator.done) | `.bss`, statics, heap | done (2026-10-03) |
| [004](../issues-done/004-apic-idt-interrupts.done) | IDT + APIC, interrupt-driven timer and keyboard | done (2026-10-03) |
| [005](../issues-done/005-syscalls.done) | `int 0x80` system calls, shared ABI | done (2026-10-03) |
| [006](../issues-done/006-bootloader-load-from-fat32.done) | Bootloader reads images from FAT | done (2026-10-03) |
| [007](../issues-done/007-kernel-font-and-primitives.done) | Font, primitives, console | done; panic diagnostics split into 018 (2026-10-03) |
| [008](../issues-done/008-kernel-timeout-handoff.done) | Handoff to userspace on timeout | superseded (2026-10-03) |
| [010](../issues-done/010-docs-sync.done) | Sync README/handoff with the code | done (2026-10-03) |
| [012](../issues-done/012-multitasking-and-program-instances.done) | Multitasking, instances, `ps`/`kill`/`fg` | done (2026-09-18) |
| [013](../issues-done/013-smp-and-memory-isolation.done) | SMP, ring 3, memory isolation | done (2026-09-19) |
| [014](../issues-done/014-private-program-heap.done) | Private program heap | done (2026-09-19) |
| [015](../issues-done/015-load-programs-through-vfs.done) | Loading programs through the VFS | done (2026-10-03) |
| [016](../issues-done/016-storage-drivers.done) | ATA / AHCI / USB storage drivers | done (2026-10-03) |
| [017](../issues-done/017-tts-on-audio-gateway.done) | Text to speech over the audio gateway | done (2026-10-03) |
| [019](../issues-done/019-platform-profile-x86-64-qemu-0.done) | Platform profile `x86-64/QEMU-0` | done (2026-10-03) |
| [020](../issues-done/020-init-bootstrap-authority.done) | `init`: bootstrap authority, driver policy out of the kernel | done (2026-10-03) |
| [021](../issues-done/021-shell-in-ring-3.done) | Shell in ring 3, input/focus policy out of the kernel | done (2026-10-03) |
| [022](../issues-done/022-audio-tools-say-listen.done) | Audio tools `say <text>`, `listen`; program arguments; AC97 capture | done (2026-10-03) |
| [023](../issues-done/023-monotonic-clock.done) | Monotonic clock with defined resolution | done (2026-10-03) |
| [024](../issues-done/024-accounted-kernel-objects.done) | Per-owner quotas for tasks and endpoints | done (2026-10-03) |
| [025](../issues-done/025-capability-generations.done) | Capability handles with generations | done (2026-10-03) |
| [026](../issues-done/026-derivation-and-revocation.done) | Capability derivation, attenuation and revocation | done (2026-10-03) |
| [027](../issues-done/027-no-ambient-endpoint-names.done) | No ambient endpoint names | done (2026-10-03) |
| [028](../issues-done/028-memory-rights-and-lease.done) | Memory rights, read-only mappings, leases ended by revoke | done (2026-10-03) |
| [029](../issues-done/029-memory-objects-move-seal.done) | Memory objects: MOVE and sealed SHARE_RO | done (2026-10-04) |
| [030](../issues-done/030-ipc-bounds-and-cancellation.done) | IPC bounds, timeouts and cancellation | done (2026-10-04) |
| [031](../issues-done/031-mind-idl-v0.done) | MIND IDL v0: WIT subset, generated bindings, receiver checks (`rtc`) | done (2026-10-04) |
| [032](../issues-done/032-program-heap.done) | Program heap `mind::alloc` | done (2026-10-04) |
| [033](../issues-done/033-font-8x16.done) | 8×16 font with Cyrillic and box drawing | done (2026-10-04) |
| [035](../issues-done/035-key-events.done) | Key events: E0 keys, modifiers, layouts, VT100 | done (2026-10-04) |
| [034](../issues-done/034-tui-library.done) | Text UI library `mind::tui` | done (2026-10-04) |
| [036](../issues-done/036-shell-line-editing.done) | Shell: line editing, history, Cyrillic | done (2026-10-04) |
| [037](../issues-done/037-viewer.done) | Viewer `view` | done (2026-10-04) |
| [038](../issues-done/038-mind-idl-v0.2.done) | MIND IDL v0.2: records, strings, lists | done (2026-10-04) |
| [039](../issues-done/039-observation-abi.done) | OBSERVE privilege, `STAT`, firmware memory map | done (2026-10-04) |
| [040](../issues-done/040-sysmon.done) | `sysmon` service | done (2026-10-04) |
| [042](../issues-done/042-loader-v1-launch-grants.done) | Loader v1: launch with granted capabilities | launch sessions (`idl/loader.wit`), `mind::request!`, console programs, `uptime` program |
| [041](../issues-done/041-top-memmap-load-hw.done) | `top`, `memmap`, `load`, `hw` | `monitor/`: `top`, `memmap`, `load`, `hw` on sysmon; `Key::latin` |
| [043](../issues-done/043-file-manager-read-only.done) | File manager `fm`, read-only | `fm`: two panels, viewer, quick view, info, find, run; VFS LIST with attributes and times |
| [044](../issues-done/044-endpoint-badges-block-write.done) | Endpoint badges and block write | endpoint badges in the kernel; BLOCK_WRITE/FLUSH for the write badge; ATA/AHCI/USB write; `block` suite |
| [045](../issues-done/045-ramdisk.done) | `ramdisk` block service | 8 MiB RAM disk service, formatted FAT16 and mounted as ram: |
| [046](../issues-done/046-vfs-v2-fat-write.done) | `vfs_server` v2: directory handles, FAT write | vfs.wit 2.0 with handles and zones; FAT12/16/32 writer with long names; write-back cache; shell file commands |
| [047](../issues-done/047-editor.done) | Editor `edit` | edit: piece table with undo, search/replace, menu and dialogs; saves via name.tmp; REQUEST_FILE lends the user's VFS client |
| [048](../issues-done/048-fm-write-df-fsck.done) | `fm` write operations, `df`, `fsck` | fm: copy/move/mkdir/delete jobs on A: and ram:, built-in editor; df; fsck (vfs.wit 2.1 check) |
| [049](../issues-done/049-logd-dmesg.done) | `logd` and `dmesg` | logd: ring with stamped sources, rate limit, read badge; println lines of services go there; dmesg; logger |

Issues 032–050 implement the [system tools plan](../docs/tools/README.md). Issues 001–011 were opened after the review of 2026-09-17 (handoff ↔ code, see [knowledge/04](../knowledge/04-handoff-vs-code-matrix.md)).
