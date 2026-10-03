# issues/ — open work items

Created from the results of the 2026-09-17 audit (handoff ↔ code comparison, see [knowledge/04](../knowledge/04-handoff-vs-code-matrix.md)).

| # | Task | Type | Priority | Blocked by |
|---|---|---|---|---|
| [001](001-flat-binary-entry-offset-and-got-call.md) | `_start` not at offset 0; `memset` through a zero GOT | bug | critical | — |
| [002](002-elf-loader.md) | ELF loader instead of flat binaries | feature (roadmap 1) | high | — |
| [003](003-bss-and-heap-allocator.md) | `.bss`, statics, `linked_list_allocator` heap | feature | high | 002 |
| [004](004-apic-idt-interrupts.md) | IDT + APIC, interrupt-driven timer and keyboard | feature (roadmap 2) | high | 003 |
| [005](005-syscalls.md) | Syscalls via `int 0x80`, shared ABI crate | feature (roadmap 3) | medium | 004 |
| [006](006-bootloader-load-from-fat32.md) | Reading kernel/app from FAT32 | feature | medium | — |
| [007](007-kernel-font-and-primitives.md) | Font, primitives, console, panic output | feature | medium | — |
| [008](008-kernel-timeout-handoff.md) | Switch to userspace on timeout | feature | low | — |
| [009](009-gop-pixel-format.md) | Honour `PixelFormat`, GOP mode selection | bug/robustness | medium | — |
| [010](010-docs-sync.md) | Sync README/handoff with the code | docs | medium | — |
| [011](011-reproducible-toolchain.md) | `rust-toolchain.toml`, `Cargo.lock`, workspace, CI | infra | low | — |
| [012](../issues-done/012-multitasking-and-program-instances.done) | Multitasking, independent instances, `ps`/`kill`/`fg` | feature — done 2026-09-18 | high | — |
| [013](../issues-done/013-smp-and-memory-isolation.done) | SMP, ring 3 and hardware memory isolation | feature — done 2026-09-19 | high | — |
| [014](../issues-done/014-private-program-heap.done) | Private dynamic memory for programs | feature — done 2026-09-19 | high | — |
| [015](../issues-done/015-load-programs-through-vfs.done) | Loading programs through the VFS instead of the bootloader RAMFS | feature — done 2026-10-03 | high | — |
| [016](../issues-done/016-storage-drivers.done) | ATA / AHCI / USB mass storage drivers for the VFS | feature — done 2026-10-03 | medium | — |
| [017](../issues-done/017-tts-on-audio-gateway.done) | Speech synthesis on top of the audio gateway | feature — done 2026-10-03 | low | — |

Recommended order: 001 → 007 (on-screen diagnostics) → 002 → 003 → 004 → 005; 006/009/010/011 in parallel.

Task file format: title, metadata block (type/priority/status/blockers), "Problem/Discrepancy", "Plan", "Acceptance criteria", "Related". On closing — status in the title and in this table.

Completed tasks 012–014 have been moved to `issues-done/` with the `.done` extension. The task statements and verification results are preserved in the files.
