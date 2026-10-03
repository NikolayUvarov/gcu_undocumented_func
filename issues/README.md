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

Issues 001–011 were opened after the review of 2026-09-17 (handoff ↔ code, see [knowledge/04](../knowledge/04-handoff-vs-code-matrix.md)).
