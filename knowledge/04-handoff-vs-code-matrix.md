# Comparison of the handoff document with the code

Legend: ✅ present in the code and matches; ⚠️ present partially or differently; ❌ absent.

## Section 1. "Current state"

| Handoff claim | Status | What is in the code | Issue |
|---|---|---|---|
| The bootloader initializes GOP and obtains the framebuffer/resolution/stride | ✅ | `bootloader/src/main.rs:35-45` | — (see the PixelFormat risk → 009) |
| The bootloader reads the kernel and the application from FAT32 | ❌ | `include_bytes!` in the EFI; the file system is not opened | [006](../issues/006-bootloader-load-from-fat32.md) |
| Flat binaries via `llvm-objcopy`, sections `.text/.rodata/.data` | ⚠️ | `-O binary` without `-j` — `.dynsym/.hash/.dynamic/.got` end up in the file | [001](../issues/001-flat-binary-entry-offset-and-got-call.md) |
| The kernel receives control from the bootloader | ⚠️ | jump to offset 0, where the kernel does not have `_start` | [001](../issues/001-flat-binary-entry-offset-and-got-call.md) |
| Basic graphics kernel: procedural rendering of a font and primitives | ❌ | only a pixel-by-pixel circle; neither a font nor primitives | [007](../issues/007-kernel-font-and-primitives.md) |
| Polling of ports `0x64`/`0x60` | ✅ | `kernel/src/main.rs:24-25` | — |
| Handoff to userspace after a delay expires **or** on a key press | ⚠️ | only on space (`0x39`); there is no timeout | [008](../issues/008-kernel-timeout-handoff.md) |
| Userspace receives the buffer pointers and takes over drawing | ✅ | `app/src/main.rs`, rotating square | — |

## Section 2. "Completed stages" (rolled back)

| Claim | Status | Comment | Issue |
|---|---|---|---|
| `linked_list_allocator`, `format!` were integrated, rolled back because of `.bss` | ❌ in the code | there are no `alloc`/allocator dependencies; no traces in the git history (2 commits) | [003](../issues/003-bss-and-heap-allocator.md) |
| "`.bss` without `linker.ld`" | ⚠️ | `linker.ld` exists and declares `.bss`; the problem is objcopy and page allocation, not a missing script | [003](../issues/003-bss-and-heap-allocator.md) |
| IDT, PIC remapping, PIT/PS2, context switching — rolled back because of a Triple Fault | ❌ in the code | there is no IDT, no `x86-interrupt` ABI, no asm switching | [004](../issues/004-apic-idt-interrupts.md) |

## Section 3. "Build infrastructure"

| Claim | Status | Comment | Issue |
|---|---|---|---|
| The `fix_stable_boot.sh` pipeline | ❌ | the file does not exist; `02_build.sh` is the current one; the README refers to a nonexistent `patch_008_preemptive.sh` | [010](../issues/010-docs-sync.md) |
| Pipeline steps 1–4 | ✅ | match `02_build.sh` (with the caveat about `-j`) | [001](../issues/001-flat-binary-entry-offset-and-got-call.md) |

## Section 4. Roadmap

| Item | Status | Issue |
|---|---|---|
| ELF loader (priority) | ❌ | [002](../issues/002-elf-loader.md) |
| APIC instead of PIC | ❌ | [004](../issues/004-apic-idt-interrupts.md) |
| Syscalls (`int 0x80`) | ❌ | [005](../issues/005-syscalls.md) |

## Not mentioned in the handoff, but found during the review

| Finding | Issue |
|---|---|
| GOP `PixelFormat` is not checked; the kernel hard-codes BGRX | [009](../issues/009-gop-pixel-format.md) |
| No `rust-toolchain.toml`, `Cargo.lock` is ignored, `01_prepare_env.sh` changes the global default | [011](../issues/011-reproducible-toolchain.md) |
| The README claims IDT and `int 0x80` as implemented | [010](../issues/010-docs-sync.md) |
