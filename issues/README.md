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

Tasks that need a person (repository settings, legal decisions, coordination of agent sessions) are in [issues-human/](../issues-human/README.md).

## Open tasks

Two tracks work in parallel. The **kernel track** owns `kernel/`, `common/abi.rs` and the core services. The **tools track** builds the user tools after its plan (`docs/tools/README.md`, branch `claude/wizardly-franklin-kec1a9`). A tools issue blocked by a kernel issue waits for it. ABI changes are made only in kernel issues. The tools branch has done 040–048 under its own numbers; [051](051-merge-main-into-tools.md) merges it and retires the rows below.

| № | Task | Type / owner | Priority | Blocked by | Roadmap |
|---|---|---|---|---|---|
| [051](051-merge-main-into-tools.md) | Merge `main` into the tools branch, reconcile duplicate IDL/STAT/input/badges and issue numbers | tools / infra | P0 | — | track G |
| [040](040-program-heap.md) | Program heap `mind::alloc` | tools | P1 | — | track G, T0 |
| [041](041-font-8x16.md) | 8×16 font with Cyrillic and box drawing | tools | P1 | — | track G, T0 |
| [042](042-tui-library.md) | TUI library `mind::tui` | tools | P1 | 040, 041 | track G, T0 |
| [043](043-keyboard-decoding-and-line-editing.md) | Keyboard decoding, layouts, VT100 input, shell line editing | tools | P1 | — | track G, T0 |
| [045](045-sysmon-and-monitors.md) | `sysmon` and `top`, `memmap`, `load`, `hw` | tools | P2 | 042 | track G, T1 |
| [046](046-loader-sessions.md) | Loader v1: launch sessions with granted capabilities | tools | P2 | — | track G, T1 |
| [047](047-viewer-and-fm-readonly.md) | Viewer `view`, file manager `fm` read-only | tools | P2 | 042, 043 | track G, T1 |
| [048](048-write-path-ramdisk-vfs2.md) | Block write, `ramdisk`, VFS v2 with directory handles | tools | P2 | — | track G, T2 |
| [049](049-editor-and-fm-write.md) | Editor `edit`, `fm` write operations, `df`, `fsck` | tools | P2 | 046, 048 | track G, T2 |
| [050](050-logd-dmesg-svc.md) | `logd`, `dmesg`, `svc` | tools | P3 | — | track G, T3 |

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
| [009](../issues-done/009-gop-pixel-format.done) | Honour `PixelFormat` and select a GOP mode | done (2026-10-04) |
| [010](../issues-done/010-docs-sync.done) | Sync README/handoff with the code | done (2026-10-03) |
| [011](../issues-done/011-reproducible-toolchain.done) | Reproducible build: pinned toolchain, lock files, CI | done (2026-10-04) |
| [012](../issues-done/012-multitasking-and-program-instances.done) | Multitasking, instances, `ps`/`kill`/`fg` | done (2026-09-18) |
| [013](../issues-done/013-smp-and-memory-isolation.done) | SMP, ring 3, memory isolation | done (2026-09-19) |
| [014](../issues-done/014-private-program-heap.done) | Private program heap | done (2026-09-19) |
| [015](../issues-done/015-load-programs-through-vfs.done) | Loading programs through the VFS | done (2026-10-03) |
| [016](../issues-done/016-storage-drivers.done) | ATA / AHCI / USB storage drivers | done (2026-10-03) |
| [017](../issues-done/017-tts-on-audio-gateway.done) | Text to speech over the audio gateway | done (2026-10-03) |
| [018](../issues-done/018-kernel-panic-diagnostics.done) | Kernel panic prints message, location, CPU and task | done (2026-10-04) |
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
| [032](../issues-done/032-minimal-supervision.done) | Minimal supervision: exit notices, restart budget, quarantine, fencing | done (2026-10-04) |
| [033](../issues-done/033-audit-and-supervision-follow-ups.done) | Follow-ups: multi-process tests, device stop, platform privilege dropped, quotas | done (2026-10-04) |
| [034](../issues-done/034-key-events-input-queue.done) | Key events in the per-task input queue | done (2026-10-04) |
| [035](../issues-done/035-observe-and-stat.done) | OBSERVE privilege, `STAT`, firmware memory map | done (2026-10-04) |
| [036](../issues-done/036-endpoint-badges.done) | Endpoint badges | done (2026-10-04) |
| [037](../issues-done/037-task-limit.done) | Task limit 32, 127 endpoints | done (2026-10-04) |
| [038](../issues-done/038-scheduling-budgets.done) | Scheduling budgets and bands (C7) | done (2026-10-04) |
| [039](../issues-done/039-port-services.done) | Port the services to MIND IDL and the C4 memory modes (C8) | done (2026-10-04) |
| [044](../issues-done/044-idl-v02-records-strings.done) | MIND IDL v0.2: records, strings, lists in buffers; `loader.wit` | done (2026-10-04) |

Issues 001–011 were opened after the review of 2026-09-17 (handoff ↔ code, see [knowledge/04](../knowledge/04-handoff-vs-code-matrix.md)).
